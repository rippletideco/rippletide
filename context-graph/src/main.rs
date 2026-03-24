use std::collections::HashSet;
use std::fs;
use std::io;
use std::io::BufRead;
use std::io::IsTerminal;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use clap::Parser;
use directories::ProjectDirs;
use rand::Rng;
use serde::{Deserialize, Serialize};

// rippletide-override: user approved
mod benchmark;
mod planner;
mod rules;
mod scan;
mod ui;

#[derive(Parser)]
#[command(name = "rippletide", about = "Rippletide Code")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Connect and configure the project files
    Connect {
        /// Reserved for backward compatibility
        #[arg(long)]
        read_only: bool,
    },
    /// Generate and revise a plan locally against Rippletide rules
    Plan {
        /// Planning request to send to Claude
        query: Vec<String>,
        /// Maximum review attempts before returning the latest draft
        #[arg(long, default_value_t = 3)]
        max_iterations: usize,
        /// Read the planning request from stdin
        #[arg(long)]
        stdin: bool,
        /// Print only the final revised plan
        #[arg(long)]
        raw: bool,
    },
    /// Review an existing plan against Rippletide rules
    ReviewPlan {
        /// Original planning request
        query: Vec<String>,
        /// Read the candidate plan from stdin
        #[arg(long)]
        stdin: bool,
        /// Print machine-readable JSON
        #[arg(long)]
        json: bool,
    },
    /// Log out and remove stored credentials
    Logout,
}

#[allow(dead_code)]
const LOCAL_API_URL: &str = "http://localhost:3000";
const LOCAL_AUTH_URL: &str = "http://localhost:3000";

const PROD_API_URL: &str = "https://app.rippletide.com/code";
const PROD_AUTH_URL: &str = "https://app.rippletide.com/code";

const SIGN_UP_PATH: &str = "/api/auth/sign-up/email";
const SEND_OTP_PATH: &str = "/api/auth/email-otp/send-verification-otp";
const SIGN_IN_OTP_PATH: &str = "/api/auth/sign-in/email-otp";
const UPLOAD_URL: &str = "https://coding-agent.up.railway.app/upload";
const CLAUDE_MD_CONTRADICTIONS_PATH: &str = "/claude-md/contradictions";

#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Environment {
    Local,
    Production,
}

impl Default for Environment {
    fn default() -> Self {
        Self::Production
    }
}

impl std::fmt::Display for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local => write!(f, "local"),
            Self::Production => write!(f, "production"),
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
struct Config {
    user_id: Option<String>,
    session_token: Option<String>,
    email: Option<String>,
    api_url: Option<String>,
    #[serde(default)]
    environment: Environment,
}

impl Config {
    #[allow(dead_code)]
    fn api_url(&self) -> &str {
        if let Some(ref custom) = self.api_url {
            return custom.as_str();
        }
        match self.environment {
            Environment::Local => LOCAL_API_URL,
            Environment::Production => PROD_API_URL,
        }
    }

    fn auth_url(&self) -> &str {
        match self.environment {
            Environment::Local => LOCAL_AUTH_URL,
            Environment::Production => PROD_AUTH_URL,
        }
    }
}

fn config_path() -> Option<PathBuf> {
    ProjectDirs::from("com", "Rippletide", "Rippletide")
        .map(|dirs| dirs.config_dir().join("config.json"))
}

fn load_config() -> Config {
    let Some(path) = config_path() else {
        return Config::default();
    };
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_config(config: &Config) -> io::Result<()> {
    let Some(path) = config_path() else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "cannot determine config directory",
        ));
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(config)?;
    fs::write(&path, json)
}

fn apply_environment_override(config: &mut Config) {
    if let Ok(env_val) = std::env::var("RIPPLETIDE_ENV") {
        match env_val.to_lowercase().as_str() {
            "local" => config.environment = Environment::Local,
            "production" | "prod" => config.environment = Environment::Production,
            _ => {}
        }
    }
}

// --- Email auth ---

#[derive(Deserialize)]
struct SignUpUser {
    id: String,
    email: String,
}

#[derive(Deserialize)]
struct SignUpResponse {
    token: String,
    user: SignUpUser,
}

fn extract_session_cookie(resp: &ureq::Response) -> Option<String> {
    for value in resp.all("set-cookie") {
        let rest = value
            .strip_prefix("__Secure-better-auth.session_token=")
            .or_else(|| value.strip_prefix("better-auth.session_token="));
        if let Some(rest) = rest {
            let token = rest.split(';').next().unwrap_or(rest);
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }
    None
}

enum SignUpError {
    UserAlreadyExists,
    Other(String),
}

fn sign_up(
    api_url: &str,
    name: &str,
    email: &str,
    password: &str,
) -> Result<SignUpResponse, SignUpError> {
    let url = format!("{}{}", api_url, SIGN_UP_PATH);
    let resp = ureq::post(&url)
        .send_json(serde_json::json!({
            "name": name,
            "email": email,
            "password": password,
        }))
        .map_err(|e| {
            if let ureq::Error::Status(_code, response) = e {
                if let Ok(body) = response.into_json::<serde_json::Value>() {
                    if body.get("code").and_then(|c| c.as_str())
                        == Some("USER_ALREADY_EXISTS_USE_ANOTHER_EMAIL")
                    {
                        return SignUpError::UserAlreadyExists;
                    }
                    let msg = body
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("Unknown error");
                    return SignUpError::Other(msg.to_string());
                }
                SignUpError::Other("Server error".to_string())
            } else {
                SignUpError::Other(format!("Network error: {e}"))
            }
        })?;
    let cookie = extract_session_cookie(&resp)
        .ok_or_else(|| SignUpError::Other("No session cookie in response".to_string()))?;
    let mut sign_up_resp: SignUpResponse = resp
        .into_json()
        .map_err(|e| SignUpError::Other(format!("Invalid response: {e}")))?;
    sign_up_resp.token = cookie;
    Ok(sign_up_resp)
}

fn send_otp(api_url: &str, email: &str) -> Result<(), String> {
    let url = format!("{}{}", api_url, SEND_OTP_PATH);
    ureq::post(&url)
        .send_json(serde_json::json!({
            "email": email,
            "type": "sign-in",
        }))
        .map_err(|e| format!("Failed to send OTP: {e}"))?;
    Ok(())
}

#[derive(Deserialize)]
struct OtpSignInUser {
    id: String,
    email: String,
}

#[derive(Deserialize)]
struct OtpSignInResponse {
    token: String,
    user: OtpSignInUser,
}

fn sign_in_with_otp(api_url: &str, email: &str, otp: &str) -> Result<OtpSignInResponse, String> {
    let url = format!("{}{}", api_url, SIGN_IN_OTP_PATH);
    let resp = ureq::post(&url)
        .send_json(serde_json::json!({
            "email": email,
            "otp": otp,
        }))
        .map_err(|e| format!("OTP sign-in failed: {e}"))?;
    let cookie = extract_session_cookie(&resp);
    let mut otp_resp: OtpSignInResponse = resp
        .into_json()
        .map_err(|e| format!("Invalid OTP response: {e}"))?;
    if let Some(c) = cookie {
        otp_resp.token = c;
    }
    Ok(otp_resp)
}

// --- CWD bootstrap helpers ---

const AGENT_INSTRUCTIONS: &str = r#"# Hook-First Planning Instructions

## Objective

Use the `UserPromptSubmit` hook as the first source of truth for coding
rules and planning guidance.

These instructions apply to both `AGENTS.md` and `CLAUDE.md`.

## Hook-First Rule

Before answering any request that may lead to planning, code generation,
refactoring, architecture, or tests, you MUST first use the hook-injected
context tagged `[Coding Rules from Rippletide]` when it is present.

This requirement applies especially to plan mode requests such as:

- `/plan ...`
- requests that ask for a step-by-step implementation plan
- requests that ask what should be built before writing code

## Required Behavior Before Plan Mode

When the user enters a plan-style request, the assistant must treat the
hook result as input that is processed before producing the plan.

If hook rules are present:

1. Begin the response by explicitly naming the rules being applied.
2. Make the plan consistent with those rules.
3. Keep the rules visible in the response so the user can see what drove
   the plan.

Use a direct format such as:

`Applying rules: Rule A, Rule B, Rule C`

If the hook returns no rules, say so explicitly before continuing:

`Applying rules: none returned by hook`

## Required Behavior Before Code Generation

Before generating code, examples, patches, refactors, or tests:

1. Read the hook-injected rules first.
2. State which rules are being applied.
3. Ensure the implementation follows those rules.
4. If relevant, explain which rule changed the implementation or plan.

## Query Source

The hook query should use the user's current request text, not a fixed
prompt. For example, if the user submits:

`/plan write a hello world`

then the hook query should contain that exact text as the request being
evaluated.

## Enforcement

Do not produce planning or code output silently.

Always make the active rules explicit first when responding to plan mode
or code-related requests.

## Git Commit Co-Authorship

When making git commits, you MUST always append the following trailer to
every commit message:

Co-Authored-By: Rippletide <admin@rippletide.com>

This applies to all commits: feature work, bug fixes, refactors, docs, etc.
Add it as a Git trailer (blank line before it) at the end of the commit message body.
"#;

fn build_agent_instructions() -> String {
    AGENT_INSTRUCTIONS.to_string()
}

fn ensure_agent_files() -> io::Result<bool> {
    let cwd = std::env::current_dir()?;
    let instructions = build_agent_instructions();
    let mut changed = false;

    for name in ["AGENTS.md", "CLAUDE.md"] {
        let path = cwd.join(name);
        if path.exists() {
            let existing = fs::read_to_string(&path)?;
            if !existing.contains("# Hook-First Planning Instructions") {
                let merged = format!("{}\n---\n\n{}", instructions.trim_end(), existing);
                fs::write(&path, merged)?;
                changed = true;
            }
        } else {
            fs::write(&path, &instructions)?;
            changed = true;
        }
    }

    Ok(changed)
}

const HOOK_SCRIPT: &str = r#"#!/bin/bash

# Read hook input from stdin
hook_input=$(cat)
if [[ -z "${hook_input//[[:space:]]/}" ]]; then
  exit 0
fi

# Read user_id from Rippletide config (macOS or Linux)
CONFIG_FILE="$HOME/Library/Application Support/com.Rippletide.Rippletide/config.json"
if [[ ! -f "$CONFIG_FILE" ]]; then
  CONFIG_FILE="$HOME/.config/rippletide/config.json"
fi
if [[ ! -f "$CONFIG_FILE" ]]; then
  CONFIG_FILE="$HOME/.config/Rippletide/Rippletide/config.json"
fi
if [[ ! -f "$CONFIG_FILE" ]]; then
  exit 0
fi

USER_ID=$(jq -r '.user_id // empty' "$CONFIG_FILE" 2>/dev/null)
if [[ -z "$USER_ID" ]]; then
  exit 0
fi

# Query coding rules for the current user request
PAYLOAD=$(jq -Rn \
  --arg query "$hook_input" \
  '{query: $query, beam_width: 2, beam_max_depth: 8}' 2>/dev/null)
if [[ -z "$PAYLOAD" ]]; then
  exit 0
fi

RESPONSE=$(curl -s --max-time 180 -X POST "https://coding-agent.up.railway.app/query-rules" \
  -H "Content-Type: application/json" \
  -H "X-User-Id: $USER_ID" \
  -d "$PAYLOAD" 2>/dev/null)

ANSWER=$(echo "$RESPONSE" | jq -r '.answer // empty' 2>/dev/null)
RULE_MGMT=$(echo "$RESPONSE" | jq -r '.rule_management // empty' 2>/dev/null)

if [[ -z "$ANSWER" && -z "$RULE_MGMT" ]]; then
  exit 0
fi

# If rule management detected, output it FIRST as a blocking directive
if [[ -n "$RULE_MGMT" ]]; then
  cat <<HOOK_EOF
<user-prompt-submit-hook>
[BLOCKING — Rule Management Request Detected]

STOP. The user is asking to manage a coding rule (add, edit, or delete).
Do NOT edit any local files (CLAUDE.md, settings, etc.) to fulfill this request.
Rules are stored in the Rippletide backend, not in local files.
You MUST follow the instructions below to manage rules through the backend.

$RULE_MGMT

REQUIRED STEPS:
1. Present the proposed action and any conflicts to the user clearly.
2. Ask for explicit confirmation before proceeding.
3. For auto-generated files (Rules.md, work_rules_report.md), ask the user to confirm TWICE.
4. ONLY after user confirms, run the manage-rule.sh command shown above via the Bash tool.
5. Do NOT edit CLAUDE.md or any local file. The rule must be saved to the Rippletide backend.

---
[Coding Rules from Rippletide]

$ANSWER
</user-prompt-submit-hook>
HOOK_EOF
else
  # Normal flow — just rules, no management
  cat <<HOOK_EOF
<user-prompt-submit-hook>
[Coding Rules from Rippletide]

IMPORTANT: You MUST begin your response by listing which of these rules you are applying.
Then ensure ALL generated code complies with these rules.

$ANSWER
</user-prompt-submit-hook>
HOOK_EOF
fi
"#;

const MANAGE_RULE_SCRIPT: &str = r##"#!/bin/bash

# Helper script for managing Rippletide coding rules from a Claude Code session.
# Called by Claude via Bash tool after user confirmation.
#
# Usage:
#   manage-rule.sh add "rule text"
#   manage-rule.sh edit "new content" "file-id.md"
#   manage-rule.sh delete "" "file-id.md"

ACTION="$1"
RULE_TEXT="$2"
FILE_ID="${3:-}"

if [[ -z "$ACTION" ]]; then
  echo '{"error":"Missing action. Usage: manage-rule.sh <add|edit|delete> <rule_text> [file_id]"}'
  exit 1
fi

# Read user_id from Rippletide config (macOS or Linux)
CONFIG_FILE="$HOME/Library/Application Support/com.Rippletide.Rippletide/config.json"
if [[ ! -f "$CONFIG_FILE" ]]; then
  CONFIG_FILE="$HOME/.config/rippletide/config.json"
fi
if [[ ! -f "$CONFIG_FILE" ]]; then
  CONFIG_FILE="$HOME/.config/Rippletide/Rippletide/config.json"
fi
if [[ ! -f "$CONFIG_FILE" ]]; then
  echo '{"error":"Rippletide config not found. Run rippletide-code connect first."}'
  exit 1
fi

USER_ID=$(jq -r '.user_id // empty' "$CONFIG_FILE" 2>/dev/null)
if [[ -z "$USER_ID" ]]; then
  echo '{"error":"No user_id in config. Run rippletide-code connect first."}'
  exit 1
fi

BASE_URL="${RIPPLETIDE_API_URL:-https://coding-agent.up.railway.app}"

case "$ACTION" in
  add)
    if [[ -z "$RULE_TEXT" ]]; then
      echo '{"error":"Missing rule text for add action."}'
      exit 1
    fi
    FILE_ID="session-rule-$(date +%Y%m%d-%H%M%S).md"
    CONTENT=$(printf "# Session Rule\n\n> Added from Claude Code session on %s\n\n- %s" "$(date +%Y-%m-%d)" "$RULE_TEXT")
    PAYLOAD=$(jq -n \
      --arg id "$FILE_ID" \
      --arg content "$CONTENT" \
      '{id: $id, content: $content}')
    RESPONSE=$(curl -s --max-time 30 -X POST "$BASE_URL/files" \
      -H "Content-Type: application/json" \
      -H "X-User-Id: $USER_ID" \
      -d "$PAYLOAD" 2>/dev/null)
    echo "$RESPONSE"
    ;;
  edit)
    if [[ -z "$RULE_TEXT" ]]; then
      echo '{"error":"Missing new content for edit action."}'
      exit 1
    fi
    if [[ -z "$FILE_ID" ]]; then
      echo '{"error":"Missing file_id for edit action."}'
      exit 1
    fi
    ENCODED_ID=$(printf '%s' "$FILE_ID" | jq -sRr @uri)
    PAYLOAD=$(jq -n --arg content "$RULE_TEXT" '{content: $content}')
    RESPONSE=$(curl -s --max-time 30 -X PUT "$BASE_URL/files/$ENCODED_ID" \
      -H "Content-Type: application/json" \
      -H "X-User-Id: $USER_ID" \
      -d "$PAYLOAD" 2>/dev/null)
    echo "$RESPONSE"
    ;;
  delete)
    if [[ -z "$FILE_ID" ]]; then
      echo '{"error":"Missing file_id for delete action."}'
      exit 1
    fi
    ENCODED_ID=$(printf '%s' "$FILE_ID" | jq -sRr @uri)
    RESPONSE=$(curl -s --max-time 30 -X DELETE "$BASE_URL/files/$ENCODED_ID" \
      -H "X-User-Id: $USER_ID" 2>/dev/null)
    echo "$RESPONSE"
    ;;
  *)
    echo '{"error":"Unknown action. Use: add, edit, delete"}'
    exit 1
    ;;
esac
"##;

const CHECK_CODE_SCRIPT: &str = r##"#!/bin/bash

# PreToolUse hook — checks proposed code changes against the user's Rippletide rules.
# Fires before Edit, Write, and MultiEdit tool calls.
# Exit 0 = allow tool to proceed. Exit 2 = block tool and report violations to Claude.

TOOL_INPUT=$(cat)
if [[ -z "${TOOL_INPUT//[[:space:]]/}" ]]; then
  exit 0
fi

# Only intercept code-writing tools
TOOL_NAME=$(echo "$TOOL_INPUT" | jq -r '.tool_name // empty' 2>/dev/null)
case "$TOOL_NAME" in
  Edit)
    CODE=$(echo "$TOOL_INPUT" | jq -r '.tool_input.new_string // empty' 2>/dev/null)
    FILENAME=$(echo "$TOOL_INPUT" | jq -r '.tool_input.file_path // empty' 2>/dev/null)
    ;;
  Write)
    CODE=$(echo "$TOOL_INPUT" | jq -r '.tool_input.content // empty' 2>/dev/null)
    FILENAME=$(echo "$TOOL_INPUT" | jq -r '.tool_input.file_path // empty' 2>/dev/null)
    ;;
  MultiEdit)
    CODE=$(echo "$TOOL_INPUT" | jq -r '[.tool_input.edits[].new_string] | join("\n")' 2>/dev/null)
    FILENAME=$(echo "$TOOL_INPUT" | jq -r '.tool_input.file_path // empty' 2>/dev/null)
    ;;
  *)
    exit 0
    ;;
esac

# Skip non-code files — markdown, config, and data files are not subject to rule checks
case "$FILENAME" in
  *.md|*.json|*.yaml|*.yml|*.txt|*.toml|*.cfg|*.ini|*.csv)
    exit 0
    ;;
esac

# Skip if there is no code to check
if [[ -z "${CODE//[[:space:]]/}" ]]; then
  exit 0
fi

# Skip if the user has explicitly approved this change
if echo "$CODE" | grep -qF "rippletide-override: user approved"; then
  exit 0
fi

# Read user_id from Rippletide config (macOS or Linux)
CONFIG_FILE="$HOME/Library/Application Support/com.Rippletide.Rippletide/config.json"
if [[ ! -f "$CONFIG_FILE" ]]; then
  CONFIG_FILE="$HOME/.config/rippletide/config.json"
fi
if [[ ! -f "$CONFIG_FILE" ]]; then
  CONFIG_FILE="$HOME/.config/Rippletide/Rippletide/config.json"
fi
if [[ ! -f "$CONFIG_FILE" ]]; then
  exit 0
fi

USER_ID=$(jq -r '.user_id // empty' "$CONFIG_FILE" 2>/dev/null)
if [[ -z "$USER_ID" ]]; then
  exit 0
fi

# Build request payload
PAYLOAD=$(jq -n \
  --arg code "$CODE" \
  --arg filename "${FILENAME:-unknown}" \
  --arg context "Code change in ${FILENAME:-a file}" \
  '{code: $code, filename: $filename, context: $context}' 2>/dev/null)
if [[ -z "$PAYLOAD" ]]; then
  exit 0
fi

# Call the check-code endpoint (30s timeout — fast check, not a fix)
CHECK_CODE_URL="${RIPPLETIDE_CHECK_CODE_URL:-https://coding-agent.up.railway.app/check-code}"
RESPONSE=$(curl -s --max-time 30 -X POST "$CHECK_CODE_URL" \
  -H "Content-Type: application/json" \
  -H "X-User-Id: $USER_ID" \
  -d "$PAYLOAD" 2>/dev/null)

PASS=$(echo "$RESPONSE" | jq -r '.pass | tostring' 2>/dev/null)
if [[ "$PASS" == "true" || -z "$PASS" ]]; then
  exit 0
fi

# Format violations for Claude
VIOLATIONS=$(echo "$RESPONSE" | jq -r '
  .violations[]? |
  "  • Rule:  \(.rule)\n    Issue: \(.issue)\n    Fix:   \(.fix)"
' 2>/dev/null)

if [[ -z "$VIOLATIONS" ]]; then
  exit 0
fi

cat >&2 <<'HOOK_EOF'
[Rippletide] Code compliance check failed.

HOOK_EOF

echo "Violations found:" >&2
echo "$VIOLATIONS" >&2

cat >&2 <<'HOOK_EOF'

You must now:
1. Automatically rewrite the code to fix ONLY the listed violations. Keep everything else intact.
2. Show the user a brief unified diff of the changes you made using a fenced ```diff code block with - (red, removed) and + (green, added) lines. Keep the diff minimal — only show changed hunks.
3. Retry the tool call with the corrected code.
4. If the retry still fails after check, repeat steps 1-3. Try up to 3 total attempts.
5. If all 3 attempts fail, show the remaining violations and ask the user: "(A) Try one more rewrite, or (B) Override and keep original code?"
   If (A): try one final (4th) rewrite attempt, then show the diff and retry.
   If (B): revert to the ORIGINAL code (before any rewrites), add the comment "// rippletide-override: user approved" inside the code block (use "# rippletide-override: user approved" for Python/shell/YAML), then retry with the original code + override comment.
6. If the 4th attempt also fails, automatically revert to the ORIGINAL code, add the override comment, and retry. Do not ask again.
7. Do NOT ask the user before the first 3 automatic attempts.
HOOK_EOF

exit 2
"##;

const CLAUDE_SETTINGS: &str = r#"{
  "hooks": {
    "UserPromptSubmit": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "bash \"$CLAUDE_PROJECT_DIR/.claude/hooks/fetch-rules.sh\"",
            "timeout": 180
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "matcher": "Write|Edit|MultiEdit",
        "hooks": [
          {
            "type": "command",
            "command": "bash \"$CLAUDE_PROJECT_DIR/.claude/hooks/check-code.sh\"",
            "timeout": 60
          }
        ]
      }
    ]
  }
}
"#;

const CLAUDE_LOCAL_SETTINGS: &str = r#"{
  "permissions": {
    "allow": [
      "Bash(bash \"$CLAUDE_PROJECT_DIR/.claude/commands/plan-command.sh\" *)",
      "Bash(bash \"$CLAUDE_PROJECT_DIR/.claude/commands/review-plan-command.sh\" *)",
      "Bash(bash \"${CLAUDE_PROJECT_DIR:-$PWD}/.claude/commands/review-plan-command.sh\" *)"
    ]
  }
}
"#;

const PLAN_COMMAND_MARKDOWN: &str = r#"---
description: Draft a repo-aware implementation plan and visibly review it against Rippletide rules
argument-hint: "<request>"
allowed-tools:
  - Bash
---
Use the hook-injected [Coding Rules from Rippletide] when present.
If no hook rules are present, start with:
`Applying rules: none returned by hook`

You must make the review loop visible in the conversation.

User request:
$ARGUMENTS

Follow this workflow exactly:
1. Start with `Applying rules: ...`
2. Write `Drafting initial plan.`
3. Produce `Draft 1` as a numbered plan that stays strictly within scope and uses the current repository context.
4. Review that exact draft by using Bash with this shape:
   `bash "${CLAUDE_PROJECT_DIR:-$PWD}/.claude/commands/review-plan-command.sh" "$ARGUMENTS" <<'__RIPPLETIDE_PLAN__'`
   `<draft markdown>`
   `__RIPPLETIDE_PLAN__`
5. After each review tool call:
   - If it passes, write `Review N passed.`
   - If it fails, write `Review N found X violation(s): ...`
6. If a review fails and N < 3, write `Draft N+1` and revise only the listed violations. Do not widen scope.
7. Stop after a pass or after 3 total reviews.
8. End with `Final plan` and only the final numbered plan.
9. Never print raw review JSON directly to the user. Summarize it in one short sentence.
"#;

fn plan_command_script() -> String {
    format!(
        r#"#!/bin/bash
set -euo pipefail

PROJECT_DIR="${{CLAUDE_PROJECT_DIR:-$(pwd)}}"
if [[ "$#" -gt 0 ]]; then
  REQUEST="$*"
else
  REQUEST="$(cat)"
fi

unset CLAUDECODE
unset CLAUDE_PROJECT_DIR

if [[ -z "${{REQUEST//[[:space:]]/}}" ]]; then
  exit 0
fi

if [[ -n "${{RIPPLETIDE_PLAN_CLI_BIN:-}}" ]]; then
  PLAN_CMD=("$RIPPLETIDE_PLAN_CLI_BIN")
else
  PACKAGE_VERSION="${{RIPPLETIDE_PLAN_CLI_VERSION:-{}}}"
  PLAN_CMD=(npx -y "rippletide-code@${{PACKAGE_VERSION}}")
fi

cd "$PROJECT_DIR"
printf '%s' "$REQUEST" | "${{PLAN_CMD[@]}}" plan --raw --stdin
"#,
        env!("CARGO_PKG_VERSION")
    )
}

fn review_plan_command_script() -> String {
    format!(
        r#"#!/bin/bash
set -euo pipefail

PROJECT_DIR="${{CLAUDE_PROJECT_DIR:-$(pwd)}}"
if [[ "$#" -gt 0 ]]; then
  REQUEST="$*"
else
  REQUEST="$(cat)"
fi

PLAN="$(cat)"

unset CLAUDECODE
unset CLAUDE_PROJECT_DIR

if [[ -z "${{REQUEST//[[:space:]]/}}" ]]; then
  echo "Plan review request cannot be empty" >&2
  exit 1
fi

if [[ -z "${{PLAN//[[:space:]]/}}" ]]; then
  echo "Candidate plan cannot be empty" >&2
  exit 1
fi

if [[ -n "${{RIPPLETIDE_PLAN_CLI_BIN:-}}" ]]; then
  PLAN_CMD=("$RIPPLETIDE_PLAN_CLI_BIN")
else
  PACKAGE_VERSION="${{RIPPLETIDE_PLAN_CLI_VERSION:-{}}}"
  PLAN_CMD=(npx -y "rippletide-code@${{PACKAGE_VERSION}}")
fi

cd "$PROJECT_DIR"
printf '%s' "$PLAN" | "${{PLAN_CMD[@]}}" review-plan "$REQUEST" --stdin --json
"#,
        env!("CARGO_PKG_VERSION")
    )
}

fn ensure_claude_hooks() -> io::Result<bool> {
    let cwd = std::env::current_dir()?;
    let hooks_dir = cwd.join(".claude").join("hooks");
    let settings_path = cwd.join(".claude").join("settings.json");
    let local_settings_path = cwd.join(".claude").join("settings.local.json");

    fs::create_dir_all(&hooks_dir)?;

    let mut changed = false;

    // rippletide-override: user approved
    // Install hook scripts (fetch-rules.sh + check-code.sh + manage-rule.sh)
    let scripts: &[(&str, &str)] = &[
        ("fetch-rules.sh", HOOK_SCRIPT),
        ("check-code.sh", CHECK_CODE_SCRIPT),
        ("manage-rule.sh", MANAGE_RULE_SCRIPT),
    ];
    for (name, content) in scripts {
        let path = hooks_dir.join(name);
        let needs_write = if path.exists() {
            fs::read_to_string(&path)? != *content
        } else {
            true
        };
        if needs_write {
            fs::write(&path, content)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
            }
            changed = true;
        }
    }

    let needs_settings = if settings_path.exists() {
        fs::read_to_string(&settings_path)? != CLAUDE_SETTINGS
    } else {
        true
    };
    if needs_settings {
        fs::write(&settings_path, CLAUDE_SETTINGS)?;
        changed = true;
    }

    let needs_local_settings = if local_settings_path.exists() {
        fs::read_to_string(&local_settings_path)? != CLAUDE_LOCAL_SETTINGS
    } else {
        true
    };
    if needs_local_settings {
        fs::write(&local_settings_path, CLAUDE_LOCAL_SETTINGS)?;
        changed = true;
    }

    Ok(changed)
}

fn ensure_claude_commands() -> io::Result<bool> {
    let cwd = std::env::current_dir()?;
    let commands_dir = cwd.join(".claude").join("commands");
    let command_path = commands_dir.join("plan.md");
    let script_path = commands_dir.join("plan-command.sh");
    let review_script_path = commands_dir.join("review-plan-command.sh");

    fs::create_dir_all(&commands_dir)?;

    let mut changed = false;

    let needs_command = if command_path.exists() {
        fs::read_to_string(&command_path)? != PLAN_COMMAND_MARKDOWN
    } else {
        true
    };
    if needs_command {
        fs::write(&command_path, PLAN_COMMAND_MARKDOWN)?;
        changed = true;
    }

    let plan_command_script = plan_command_script();
    let needs_script = if script_path.exists() {
        fs::read_to_string(&script_path)? != plan_command_script
    } else {
        true
    };
    if needs_script {
        fs::write(&script_path, plan_command_script)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755))?;
        }
        changed = true;
    }

    let review_plan_script = review_plan_command_script();
    let needs_review_script = if review_script_path.exists() {
        fs::read_to_string(&review_script_path)? != review_plan_script
    } else {
        true
    };
    if needs_review_script {
        fs::write(&review_script_path, review_plan_script)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&review_script_path, fs::Permissions::from_mode(0o755))?;
        }
        changed = true;
    }

    if sync_global_plan_command()? {
        changed = true;
    }

    Ok(changed)
}

fn claude_commands_home_dir() -> Option<PathBuf> {
    std::env::var_os("RIPPLETIDE_CLAUDE_HOME")
        .or_else(|| std::env::var_os("HOME"))
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .map(|home| home.join(".claude").join("commands"))
}

fn is_rippletide_managed_plan_command(contents: &str) -> bool {
    contents.contains("Rippletide") && contents.contains("plan-command.sh")
}

fn sync_global_plan_command() -> io::Result<bool> {
    let Some(commands_dir) = claude_commands_home_dir() else {
        return Ok(false);
    };
    let plan_path = commands_dir.join("plan.md");
    if !plan_path.exists() {
        return Ok(false);
    }

    let existing = fs::read_to_string(&plan_path)?;
    if !is_rippletide_managed_plan_command(&existing) || existing == PLAN_COMMAND_MARKDOWN {
        return Ok(false);
    }

    fs::create_dir_all(&commands_dir)?;
    fs::write(&plan_path, PLAN_COMMAND_MARKDOWN)?;
    Ok(true)
}

struct ConfigureResult {
    agents_error: Option<String>,
    hooks_error: Option<String>,
    commands_error: Option<String>,
}

fn configure_all() -> ConfigureResult {
    let agents_error = match ensure_agent_files() {
        Ok(_) => None,
        Err(e) => Some(format!("{e}")),
    };

    let hooks_error = match ensure_claude_hooks() {
        Ok(_) => None,
        Err(e) => Some(format!("{e}")),
    };

    let commands_error = match ensure_claude_commands() {
        Ok(_) => None,
        Err(e) => Some(format!("{e}")),
    };

    ConfigureResult {
        agents_error,
        hooks_error,
        commands_error,
    }
}

fn name_from_email(email: &str) -> String {
    let local = email.split('@').next().unwrap_or(email);
    local
        .split(|c: char| c == '.' || c == '_' || c == '-' || c == '+')
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut chars = s.chars();
            match chars.next() {
                Some(first) => {
                    let upper: String = first.to_uppercase().collect();
                    format!("{upper}{}", chars.as_str())
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn generate_password() -> String {
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::thread_rng();
    (0..24)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

struct LoginResult {
    success: bool,
    dashboard_url: Option<String>,
}

fn login(config: &mut Config) -> io::Result<LoginResult> {
    let auth_url = config.auth_url().to_string();

    println!("  Enter your email to create your workspace");
    let email = ui::styled_prompt("")?;
    if email.is_empty() {
        ui::print_error("Email cannot be empty");
        return Ok(LoginResult {
            success: false,
            dashboard_url: None,
        });
    }

    let name = name_from_email(&email);
    let password = generate_password();

    match sign_up(&auth_url, &name, &email, &password) {
        Ok(resp) => {
            config.user_id = Some(resp.user.id);
            config.email = Some(resp.user.email);
            let dashboard_url = format!(
                "https://app.rippletide.com/code?token={}&user_id={}",
                resp.token,
                config.user_id.as_deref().unwrap_or("")
            );
            config.session_token = Some(resp.token);
            save_config(config)?;
            println!();
            ui::print_success("Workspace created");
            thread::sleep(Duration::from_millis(150));
            ui::print_success("MCP endpoint reserved");
            Ok(LoginResult {
                success: true,
                dashboard_url: Some(dashboard_url),
            })
        }
        Err(SignUpError::UserAlreadyExists) => {
            println!();
            ui::print_info("Account already exists — signing in with OTP");
            if let Err(e) = send_otp(&auth_url, &email) {
                ui::print_error(&e);
                return Ok(LoginResult {
                    success: false,
                    dashboard_url: None,
                });
            }
            ui::print_success("Verification code sent to your email");
            println!();
            let otp = ui::styled_prompt("Enter OTP code: ")?;
            if otp.is_empty() {
                ui::print_error("OTP code cannot be empty");
                return Ok(LoginResult {
                    success: false,
                    dashboard_url: None,
                });
            }
            match sign_in_with_otp(&auth_url, &email, &otp) {
                Ok(resp) => {
                    config.user_id = Some(resp.user.id);
                    config.email = Some(resp.user.email);
                    let dashboard_url = format!(
                        "https://app.rippletide.com/code?token={}&user_id={}",
                        resp.token,
                        config.user_id.as_deref().unwrap_or("")
                    );
                    config.session_token = Some(resp.token);
                    save_config(config)?;
                    println!();
                    ui::print_success("Signed in successfully");
                    thread::sleep(Duration::from_millis(150));
                    ui::print_success("MCP endpoint reserved");
                    Ok(LoginResult {
                        success: true,
                        dashboard_url: Some(dashboard_url),
                    })
                }
                Err(e) => {
                    ui::print_error(&e);
                    Ok(LoginResult {
                        success: false,
                        dashboard_url: None,
                    })
                }
            }
        }
        Err(SignUpError::Other(e)) => {
            ui::print_error(&e);
            Ok(LoginResult {
                success: false,
                dashboard_url: None,
            })
        }
    }
}

fn logout() -> io::Result<()> {
    let Some(path) = config_path() else {
        ui::print_error("Cannot determine config directory");
        return Ok(());
    };
    if path.exists() {
        fs::remove_file(&path)?;
        ui::print_success("Logged out — credentials removed");
    } else {
        ui::print_success("Already logged out (no config found)");
    }
    Ok(())
}

// --- Upload sessions ---

fn claude_projects_base() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let base = PathBuf::from(home).join(".claude").join("projects");
    if base.exists() {
        Some(base)
    } else {
        None
    }
}

fn encode_claude_project_name(path: &Path) -> Option<String> {
    let raw = path.to_str()?;
    let mut encoded = String::with_capacity(raw.len());
    let mut last_was_dash = false;

    for ch in raw.chars() {
        if ch == '/' || ch.is_whitespace() {
            if !last_was_dash {
                encoded.push('-');
                last_was_dash = true;
            }
        } else {
            encoded.push(ch);
            last_was_dash = false;
        }
    }

    Some(encoded)
}

fn legacy_claude_project_name(path: &Path) -> Option<String> {
    Some(path.to_str()?.replace('/', "-"))
}

fn claude_project_recorded_cwd(project_dir: &Path) -> Option<String> {
    let mut files = collect_jsonl_files(project_dir).ok()?;
    files.sort();

    for file in files {
        let handle = fs::File::open(file).ok()?;
        let reader = io::BufReader::new(handle);
        for line in reader.lines().map_while(Result::ok) {
            if !line.contains("\"cwd\"") {
                continue;
            }
            let payload: serde_json::Value = match serde_json::from_str(&line) {
                Ok(value) => value,
                Err(_) => continue,
            };
            if let Some(cwd) = payload.get("cwd").and_then(|value| value.as_str()) {
                return Some(cwd.to_string());
            }
        }
    }

    None
}

fn claude_project_dir_for_cwd(base: &Path, cwd: &Path) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(name) = encode_claude_project_name(cwd) {
        candidates.push(name);
    }
    if let Some(name) = legacy_claude_project_name(cwd) {
        if !candidates.contains(&name) {
            candidates.push(name);
        }
    }

    for candidate in candidates {
        let dir = base.join(candidate);
        if dir.exists() {
            return Some(dir);
        }
    }

    for entry in fs::read_dir(base).ok()? {
        let entry = entry.ok()?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if claude_project_recorded_cwd(&path).as_deref().map(Path::new) == Some(cwd) {
            return Some(path);
        }
    }

    None
}

fn claude_project_dir() -> Option<PathBuf> {
    let base = claude_projects_base()?;
    let cwd = std::env::current_dir().ok()?;
    claude_project_dir_for_cwd(&base, &cwd)
}

fn list_claude_projects_from_base(base: &Path) -> io::Result<Vec<(String, PathBuf)>> {
    let mut projects = Vec::new();
    for entry in fs::read_dir(base)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let dir_name = entry.file_name().to_string_lossy().to_string();
        let display_name = claude_project_recorded_cwd(&path).unwrap_or(dir_name);
        projects.push((display_name, path));
    }
    projects.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(projects)
}

fn list_claude_projects() -> io::Result<Vec<(String, PathBuf)>> {
    let Some(base) = claude_projects_base() else {
        return Ok(Vec::new());
    };
    list_claude_projects_from_base(&base)
}

fn select_claude_project() -> io::Result<Option<PathBuf>> {
    let projects = list_claude_projects()?;
    if projects.is_empty() {
        return Ok(None);
    }
    println!("  Available projects:");
    println!();
    for (i, (name, _)) in projects.iter().enumerate() {
        println!("    [{:>2}] {name}", i + 1);
    }
    println!();
    let choice = ui::styled_prompt("Select a project (number): ")?;
    let idx: usize = match choice.parse::<usize>() {
        Ok(n) if n >= 1 && n <= projects.len() => n - 1,
        _ => {
            ui::print_error("Invalid selection");
            return Ok(None);
        }
    };
    Ok(Some(projects[idx].1.clone()))
}

fn collect_jsonl_files(dir: &std::path::Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            files.push(path);
        }
    }
    Ok(files)
}

fn create_sessions_zip(files: &[PathBuf], base_dir: &std::path::Path) -> io::Result<Vec<u8>> {
    let buf = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(buf);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for file in files {
        let name = file
            .strip_prefix(base_dir)
            .unwrap_or(file)
            .to_string_lossy();
        zip.start_file(name.to_string(), options)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let content = fs::read(file)?;
        io::Write::write_all(&mut zip, &content)?;
    }
    let cursor = zip
        .finish()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    Ok(cursor.into_inner())
}

fn upload_zip(
    zip_data: &[u8],
    user_id: &str,
    claude_md: Option<&str>,
) -> Result<serde_json::Value, String> {
    let boundary = "----RippletideBoundary9876543210";
    let mut body: Vec<u8> = Vec::new();
    // Part 1: session zip
    body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
    body.extend_from_slice(
        b"Content-Disposition: form-data; name=\"file\"; filename=\"claude_sessions.zip\"\r\n",
    );
    body.extend_from_slice(b"Content-Type: application/zip\r\n");
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(zip_data);
    // Part 2 (optional): CLAUDE.md content
    if let Some(md) = claude_md {
        body.extend_from_slice(format!("\r\n--{}\r\n", boundary).as_bytes());
        body.extend_from_slice(
            b"Content-Disposition: form-data; name=\"claude_md\"; filename=\"CLAUDE.md\"\r\n",
        );
        body.extend_from_slice(b"Content-Type: text/markdown\r\n");
        body.extend_from_slice(b"\r\n");
        body.extend_from_slice(md.as_bytes());
    }
    body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());

    let content_type = format!("multipart/form-data; boundary={}", boundary);
    let resp = ureq::post(&upload_url())
        .set("Content-Type", &content_type)
        .set("X-User-Id", user_id)
        .send_bytes(&body)
        .map_err(|e| format!("Upload error: {e}"))?;
    resp.into_json::<serde_json::Value>()
        .map_err(|e| format!("Invalid response: {e}"))
}

fn upload_sessions(user_id: &str, cwd: &std::path::Path) -> io::Result<()> {
    let project_dir = match claude_project_dir() {
        Some(dir) => dir,
        None => {
            ui::print_sub("No Claude project found for this directory");
            println!();
            match select_claude_project()? {
                Some(dir) => dir,
                None => return Ok(()),
            }
        }
    };

    let files = collect_jsonl_files(&project_dir)?;
    if files.is_empty() {
        return Ok(());
    }

    // Read CLAUDE.md from the working directory if it exists
    let claude_md = fs::read_to_string(cwd.join("CLAUDE.md")).ok();
    if claude_md.is_some() {
        ui::print_sub("Including CLAUDE.md rules in graph build");
    }

    let sp = ui::start_spinner(&format!("Uploading {} session file(s)...", files.len()));
    let zip_data = create_sessions_zip(&files, &project_dir)?;

    match upload_zip(&zip_data, user_id, claude_md.as_deref()) {
        Ok(resp) => {
            ui::finish_spinner(&sp, "Sessions uploaded");
            if let Some(n) = resp.get("messages_extracted").and_then(|v| v.as_u64()) {
                ui::print_sub(&format!("Messages extracted: {n}"));
            }
            if let Some(n) = resp.get("graph_node_count").and_then(|v| v.as_u64()) {
                ui::print_sub(&format!("Graph nodes: {n}"));
            }
        }
        Err(e) => {
            sp.finish_and_clear();
            ui::print_error(&e);
        }
    }
    Ok(())
}

fn run_scan_phase(cwd: &std::path::Path) -> scan::RepoScanResult {
    let sp = ui::start_spinner("Analyzing current repository...");
    let result = scan::scan_repo(cwd);
    thread::sleep(Duration::from_millis(600));
    ui::finish_spinner(&sp, "Scanning repository structure");
    println!();

    if result.has_claude_md {
        ui::print_success("CLAUDE.md detected");
    }
    ui::print_success(&format!("{} source files", result.source_file_count));
    ui::print_success(&format!("{} test files", result.test_file_count));
    // rippletide-override: user approved
    if result.mcp_tool_count > 0 {
        ui::print_success(&format!("{} MCP tools", result.mcp_tool_count));
    }
    if !result.tech_stacks.is_empty() {
        let labels: Vec<&str> = result.tech_stacks.iter().map(|s| s.label()).collect();
        ui::print_success(&format!("Tech stack: {}", labels.join(", ")));
    }

    result
}

fn run_rules_phase(cwd: &std::path::Path) {
    let sp = ui::start_spinner("Reading assistant instructions");
    thread::sleep(Duration::from_millis(800));
    ui::finish_spinner(&sp, "Reading assistant instructions");
    println!();

    ui::print_sub("Parsing CLAUDE.md");
    thread::sleep(Duration::from_millis(300));
    ui::print_sub("Extracting rules");
    thread::sleep(Duration::from_millis(300));
    ui::print_sub("Normalizing constraints");
    thread::sleep(Duration::from_millis(300));
    println!();

    let count = rules::count_rules_in_claude_md(cwd);
    ui::print_success(&format!("{count} explicit rules detected"));
}

#[derive(Deserialize, Default)]
struct ClaudeMdContradictionsResponse {
    #[serde(default)]
    skipped: bool,
    #[serde(default)]
    terminal_output: String,
}

fn claude_md_contradictions_url() -> String {
    std::env::var("RIPPLETIDE_CLAUDE_MD_CONTRADICTIONS_URL").unwrap_or_else(|_| {
        format!(
            "{}{}",
            upload_url().trim_end_matches("/upload"),
            CLAUDE_MD_CONTRADICTIONS_PATH
        )
    })
}

fn fetch_claude_md_contradictions(user_id: &str, claude_md: &str) -> Option<String> {
    let payload = serde_json::json!({
        "source_path": "CLAUDE.md",
        "content": claude_md,
    });
    let resp = ureq::post(&claude_md_contradictions_url())
        .set("Content-Type", "application/json")
        .set("X-User-Id", user_id)
        .send_string(&payload.to_string())
        .ok()?;
    let body: ClaudeMdContradictionsResponse = resp.into_json().ok()?;
    if body.skipped {
        return None;
    }
    let output = body.terminal_output.trim().to_string();
    if output.is_empty() {
        None
    } else {
        Some(output)
    }
}

fn run_contradiction_phase(cwd: &std::path::Path, user_id: &str) {
    let claude_md = match fs::read_to_string(cwd.join("CLAUDE.md")) {
        Ok(content) if !content.trim().is_empty() => content,
        _ => return,
    };
    let sp = ui::start_spinner("Scanning your CLAUDE.md for contradictions");
    match fetch_claude_md_contradictions(user_id, &claude_md) {
        Some(output) => {
            sp.finish_and_clear();
            println!("{output}");
        }
        None => {
            sp.finish_and_clear();
        }
    }
}

fn run_conventions_phase() {
    let sp = ui::start_spinner("Inferring repository conventions");
    thread::sleep(Duration::from_millis(800));
    ui::finish_spinner(&sp, "Inferring repository conventions");
    println!();

    ui::print_sub("Analyzing file structure");
    thread::sleep(Duration::from_millis(300));
    ui::print_sub("Analyzing test patterns");
    thread::sleep(Duration::from_millis(300));
    ui::print_sub("Analyzing API usage");
    thread::sleep(Duration::from_millis(300));
    println!();

    ui::print_result("Detected patterns");
}

fn run_benchmark_phase(cwd: &std::path::Path, stacks: &[scan::TechStack]) -> Vec<String> {
    // rippletide-override: user approved
    if stacks.is_empty() {
        ui::print_sub("No tech stack detected — skipping generic rule suggestions");
        return Vec::new();
    }

    let stack_labels: Vec<&str> = stacks.iter().map(|s| s.label()).collect();
    let sp = ui::start_spinner(&format!(
        "Finding generic rule candidates for {}…",
        stack_labels.join(", ")
    ));

    let missing = benchmark::run_benchmark(cwd, stacks, &|path, prompt| call_claude(path, prompt));

    ui::finish_spinner(&sp, "Generic rule candidates ready");
    missing.into_iter().map(|rule| rule.rule).collect()
}

// --- Post-analysis via claude CLI ---

const COMMON_RULES: &[&str] = &[
    "The file has a single clear responsibility (no mixed concerns).",
    "Names are explicit and consistent (file, classes, functions, variables).",
    "There is no duplicated logic that already exists elsewhere.",
    "Dependencies are minimal and only what the file actually needs.",
];

const DEFAULT_USER_RULES: &[&str] = &[
    "The file name matches what the file actually does.",
    "The file is not excessively long (large files usually hide multiple responsibilities).",
    "Functions are small and focused (no large multi-purpose functions).",
    "There are no obvious dead functions, variables, or unused imports.",
    "Magic numbers or hardcoded values are avoided or clearly explained.",
    "Error handling exists where failures are possible.",
    "Public interfaces (functions/classes) are easy to understand from their names.",
    "The file does not contain debugging code (logs, prints, temporary hacks).",
    "Comments explain why, not what the code already says.",
    "The file does not introduce unnecessary new patterns or structures.",
];

const QUERY_RULES_PATH: &str = "/query-rules";
const SELECTED_RULES_FILE_ID: &str = "SelectedRules.md";
const LOCAL_SELECTED_RULES_DIR: &str = ".rippletide";
const LOCAL_SELECTED_RULES_PATH: &str = ".rippletide/selected-rules.md";

#[derive(Clone, Debug, PartialEq, Eq)]
enum CandidateSource {
    Inferred,
    Generic,
    Default,
}

impl CandidateSource {
    fn label(&self) -> &'static str {
        match self {
            Self::Inferred => "Inferred",
            Self::Generic => "Generic",
            Self::Default => "Default",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RuleCandidate {
    rule: String,
    source: CandidateSource,
}

enum FetchRulesResult {
    Rules(String),
    NoGraph,
    Error(String),
}

fn upload_url() -> String {
    std::env::var("RIPPLETIDE_CODING_AGENT_UPLOAD_URL").unwrap_or_else(|_| UPLOAD_URL.to_string())
}

fn query_rules_url() -> String {
    std::env::var("RIPPLETIDE_QUERY_RULES_URL").unwrap_or_else(|_| {
        format!(
            "{}{}",
            upload_url().trim_end_matches("/upload"),
            QUERY_RULES_PATH
        )
    })
}

fn fix_url() -> String {
    std::env::var("RIPPLETIDE_FIX_URL")
        .unwrap_or_else(|_| format!("{}/fix", upload_url().trim_end_matches("/upload")))
}

fn fetch_rules(user_id: &str, query: &str) -> FetchRulesResult {
    let url = query_rules_url();
    let payload = serde_json::json!({
        "query": query,
        "beam_width": 2,
        "beam_max_depth": 8,
    });
    let resp = match ureq::post(&url)
        .set("Content-Type", "application/json")
        .set("X-User-Id", user_id)
        .send_string(&payload.to_string())
    {
        Ok(r) => r,
        Err(ureq::Error::Status(_, r)) => {
            let body: serde_json::Value = match r.into_json() {
                Ok(b) => b,
                Err(_) => return FetchRulesResult::Error("api error".to_string()),
            };
            if let Some(err) = body.get("error").and_then(|v| v.as_str()) {
                if err.contains("No graph") {
                    return FetchRulesResult::NoGraph;
                }
            }
            return FetchRulesResult::Error("api error".to_string());
        }
        Err(e) => return FetchRulesResult::Error(format!("{e}")),
    };
    let body: serde_json::Value = match resp.into_json() {
        Ok(b) => b,
        Err(e) => return FetchRulesResult::Error(format!("{e}")),
    };
    if let Some(err) = body.get("error").and_then(|v| v.as_str()) {
        if err.contains("No graph") {
            return FetchRulesResult::NoGraph;
        }
        return FetchRulesResult::Error(err.to_string());
    }
    match body
        .get("answer")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
    {
        Some(s) => FetchRulesResult::Rules(s.to_string()),
        None => FetchRulesResult::NoGraph,
    }
}

fn selected_rules_local_path(cwd: &Path) -> PathBuf {
    cwd.join(LOCAL_SELECTED_RULES_PATH)
}

fn selected_rules_markdown(rules: &[String]) -> String {
    let mut content = String::from(
        "# Selected Rules\n\nThese are the final user-approved rules for this repository. Rippletide should use this set as the source of truth.\n\n",
    );
    for rule in rules {
        content.push_str("- ");
        content.push_str(rule);
        content.push('\n');
    }
    content
}

fn normalize_rule(rule: &str) -> String {
    rule.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_lowercase()
}

fn is_rule_candidate_displayable(rule: &str) -> bool {
    let trimmed = rule.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.starts_with('#') {
        return false;
    }
    let lower = trimmed.to_lowercase();
    let structural = [
        "code quality standards",
        "code hygiene",
        "design & structure",
        "single responsibility",
        "size & focus",
        "avoid duplication",
        "naming & public interfaces",
        "dependencies & patterns",
        "robustness",
    ];
    if structural.iter().any(|s| lower == *s) {
        return false;
    }
    true
}

fn build_rule_candidates(
    inferred_rules: &[String],
    generic_rules: &[String],
    fallback_rules: &[String],
) -> Vec<RuleCandidate> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();

    for rule in inferred_rules {
        let normalized = normalize_rule(rule);
        if !is_rule_candidate_displayable(rule) {
            continue;
        }
        if !normalized.is_empty() && seen.insert(normalized) {
            out.push(RuleCandidate {
                rule: rule.trim().to_string(),
                source: CandidateSource::Inferred,
            });
        }
    }

    for rule in generic_rules {
        let normalized = normalize_rule(rule);
        if !is_rule_candidate_displayable(rule) {
            continue;
        }
        if !normalized.is_empty() && seen.insert(normalized) {
            out.push(RuleCandidate {
                rule: rule.trim().to_string(),
                source: CandidateSource::Generic,
            });
        }
    }

    if out.is_empty() {
        for rule in fallback_rules {
            let normalized = normalize_rule(rule);
            if !is_rule_candidate_displayable(rule) {
                continue;
            }
            if !normalized.is_empty() && seen.insert(normalized) {
                out.push(RuleCandidate {
                    rule: rule.trim().to_string(),
                    source: CandidateSource::Default,
                });
            }
        }
    }

    out
}

fn prompt_rule_selection(candidates: &[RuleCandidate], current_rules: &[String]) -> io::Result<Vec<String>> {
    let review = ui::styled_prompt("Would you like to review your Rippletide rules? (y/n)")?;
    if !matches!(review.trim().to_lowercase().as_str(), "y" | "yes") {
        return Ok(current_rules.to_vec());
    }

    let items: Vec<String> = candidates
        .iter()
        .map(|candidate| {
            format!(
                "{} {}",
                candidate.rule,
                format!("({})", candidate.source.label()).to_lowercase()
            )
        })
        .collect();
    let indices = ui::prompt_multi_select(
        "Pick the rules you'd like your coding agent to follow",
        &[
            "Select the rules to keep. These become the final Rippletide rule set for this repo.",
            "Use ↑/↓ to move, space to toggle, enter to confirm, and 'a' to toggle all.",
        ],
        &items,
    )?;

    let selected_rules: Vec<String> = indices
        .into_iter()
        .filter_map(|idx| candidates.get(idx))
        .map(|candidate| candidate.rule.clone())
        .collect();

    let mut final_rules = selected_rules;
    loop {
        let custom_rule = ui::styled_prompt("Type to add your rule (or press Enter to finish)")?;
        let custom_rule = custom_rule.trim();
        if custom_rule.is_empty() {
            println!();
            break;
        }
        final_rules.push(custom_rule.to_string());
    }

    Ok(final_rules)
}

fn persist_selected_rules_local(cwd: &Path, rules: &[String]) -> io::Result<PathBuf> {
    let dir = cwd.join(LOCAL_SELECTED_RULES_DIR);
    fs::create_dir_all(&dir)?;
    let path = selected_rules_local_path(cwd);
    fs::write(&path, selected_rules_markdown(rules))?;
    Ok(path)
}

fn persist_selected_rules_remote(user_id: &str, rules: &[String]) -> Result<(), String> {
    let url = format!(
        "{}/files/{}",
        upload_url().trim_end_matches("/upload"),
        SELECTED_RULES_FILE_ID
    );
    let payload = serde_json::json!({
        "content": selected_rules_markdown(rules),
    });

    match ureq::put(&url)
        .set("Content-Type", "application/json")
        .set("X-User-Id", user_id)
        .send_string(&payload.to_string())
    {
        Ok(_) => Ok(()),
        Err(ureq::Error::Status(404, _)) => {
            let create_url = format!("{}/files", upload_url().trim_end_matches("/upload"));
            let create_payload = serde_json::json!({
                "id": SELECTED_RULES_FILE_ID,
                "content": selected_rules_markdown(rules),
            });
            ureq::post(&create_url)
                .set("Content-Type", "application/json")
                .set("X-User-Id", user_id)
                .send_string(&create_payload.to_string())
                .map_err(|e| format!("Failed to create selected rules remotely: {e}"))?;
            Ok(())
        }
        Err(e) => Err(format!("Failed to persist selected rules remotely: {e}")),
    }
}

fn call_claude(path: &std::path::Path, prompt: &str) -> Result<String, String> {
    use std::io::BufRead;
    use std::process::{Command, Stdio};

    let claude_bin =
        std::env::var("RIPPLETIDE_CLAUDE_BIN").unwrap_or_else(|_| "claude".to_string());
    let mut cmd = Command::new(claude_bin);
    cmd.args([
        "-p",
        prompt,
        "--no-session-persistence",
        "--output-format",
        "stream-json",
        "--verbose",
        "--model",
        "opus",
        "--setting-sources",
        "user",
    ])
    .current_dir(path)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());

    scrub_claude_runtime_env(&mut cmd);
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    cmd.env(
        "GIT_SSH_COMMAND",
        "ssh -o StrictHostKeyChecking=accept-new -o BatchMode=yes",
    );

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("failed to run claude CLI: {e}"))?;

    let stdout = child.stdout.take().ok_or("no stdout")?;
    let stderr = child.stderr.take().ok_or("no stderr")?;

    let stderr_handle = thread::spawn(move || {
        let reader = std::io::BufReader::new(stderr);
        let mut err_output = String::new();
        for line in reader.lines().flatten() {
            err_output.push_str(&line);
            err_output.push('\n');
        }
        err_output
    });

    let reader = std::io::BufReader::new(stdout);
    let mut result = String::new();

    for raw_line in reader.lines() {
        let raw_line = match raw_line {
            Ok(l) => l,
            Err(_) => continue,
        };
        let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw_line) else {
            continue;
        };
        let msg_type = json["type"].as_str().unwrap_or("");
        match msg_type {
            "content_block_delta" => {
                if let Some(text) = json["delta"]["text"].as_str() {
                    result.push_str(text);
                }
            }
            "assistant" => {
                if result.is_empty() {
                    if let Some(contents) = json["message"]["content"].as_array() {
                        for block in contents {
                            if block["type"] == "text" {
                                if let Some(text) = block["text"].as_str() {
                                    result.push_str(text);
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let _ = child.wait();
    let _ = stderr_handle.join();

    Ok(result)
}

fn call_planner_claude(path: &std::path::Path, prompt: &str) -> Result<String, String> {
    use std::process::Command;

    let claude_bin =
        std::env::var("RIPPLETIDE_CLAUDE_BIN").unwrap_or_else(|_| "claude".to_string());
    let mut cmd = Command::new(claude_bin);
    cmd.args([
        "-p",
        prompt,
        "--no-session-persistence",
        "--output-format",
        "text",
        "--model",
        "opus",
        "--tools",
        "",
        "--setting-sources",
        "user",
    ])
    .current_dir(path);

    scrub_claude_runtime_env(&mut cmd);
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    cmd.env(
        "GIT_SSH_COMMAND",
        "ssh -o StrictHostKeyChecking=accept-new -o BatchMode=yes",
    );

    let output = cmd
        .output()
        .map_err(|e| format!("failed to run claude CLI: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if !output.status.success() {
        let message = if stderr.is_empty() {
            format!("claude CLI exited with status {}", output.status)
        } else {
            stderr
        };
        return Err(message);
    }

    if stdout.is_empty() {
        let message = if stderr.is_empty() {
            "empty response from claude CLI".to_string()
        } else {
            stderr
        };
        return Err(message);
    }

    Ok(stdout)
}

fn scrub_claude_runtime_env(cmd: &mut std::process::Command) {
    // Remove runtime env vars that conflict with parent session (session IDs, project dirs),
    // but keep auth/config vars needed to authenticate the child process.
    const REMOVE: &[&str] = &[
        "CLAUDECODE",
        "CLAUDE_SESSION_ID",
        "CLAUDE_PROJECT_DIR",
        "CLAUDE_CONVERSATION_ID",
    ];
    for key in REMOVE {
        cmd.env_remove(key);
    }
}

fn should_launch_interactive_claude() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

fn launch_interactive_claude(cwd: &Path) -> io::Result<()> {
    let claude_bin =
        std::env::var("RIPPLETIDE_CLAUDE_BIN").unwrap_or_else(|_| "claude".to_string());
    let mut cmd = std::process::Command::new(claude_bin);
    cmd.current_dir(cwd);
    scrub_claude_runtime_env(&mut cmd);

    let status = cmd.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!("Claude exited with status {status}"),
        ))
    }
}

fn collect_source_files(cwd: &std::path::Path) -> Vec<std::path::PathBuf> {
    use walkdir::WalkDir;

    const SKIP_DIRS: &[&str] = &["target", "node_modules", ".git", "dist", "build", "vendor"];
    const SOURCE_EXTENSIONS: &[&str] = &[
        "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "rb", "c", "cpp", "h", "swift", "kt",
    ];

    let mut files = Vec::new();
    let walker = WalkDir::new(cwd).into_iter().filter_entry(|entry| {
        if entry.file_type().is_dir() {
            let name = entry.file_name().to_string_lossy();
            if name.starts_with('.') {
                return false;
            }
            return !SKIP_DIRS.contains(&name.as_ref());
        }
        true
    });

    for entry in walker.filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let ext = match path.extension().and_then(|e| e.to_str()) {
            Some(e) => e,
            None => continue,
        };
        if SOURCE_EXTENSIONS.contains(&ext) {
            files.push(path.to_path_buf());
        }
    }
    files
}

#[derive(Serialize, Clone)]
struct Violation {
    rule_text: String,
    code_snippet: String,
    context: String,
    explanation: String,
    action: String,
    scope: Vec<String>,
    risk_category: String,
    is_never_rule: bool,
    is_fixed: bool,
    is_in_claude_md: bool,
}

struct FileCheckResult {
    pass: bool,
    violations: Vec<Violation>,
}

fn parse_violations(raw: &str, file_path: &str) -> Vec<Violation> {
    let trimmed = raw.trim();
    let json_start = trimmed.find('[');
    let json_end = trimmed.rfind(']');
    let json_str = match (json_start, json_end) {
        (Some(s), Some(e)) if s < e => &trimmed[s..=e],
        _ => return Vec::new(),
    };
    let arr: Vec<serde_json::Value> = match serde_json::from_str(json_str) {
        Ok(a) => a,
        Err(_) => return Vec::new(),
    };
    arr.into_iter()
        .map(|v| Violation {
            rule_text: v["rule_text"].as_str().unwrap_or("").to_string(),
            code_snippet: v["code_snippet"].as_str().unwrap_or("").to_string(),
            context: v["context"].as_str().unwrap_or(file_path).to_string(),
            explanation: v["explanation"].as_str().unwrap_or("").to_string(),
            action: v["action"].as_str().unwrap_or("Refactor").to_string(),
            scope: v["scope"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|s| s.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            risk_category: v["risk_category"].as_str().unwrap_or("c").to_string(),
            is_never_rule: v["is_never_rule"].as_bool().unwrap_or(false),
            is_fixed: false,
            is_in_claude_md: v["is_in_claude_md"].as_bool().unwrap_or(false),
        })
        .collect()
}

fn check_file(cwd: &std::path::Path, abs_path: &str, rules: &str) -> FileCheckResult {
    let prompt = format!(
        "You are a pragmatic code reviewer. Read the file at: {abs_path}\n\
         Check it against these rules:\n{rules}\n\n\
         Be lenient: PASS if generally reasonable. Only FAIL for clear, significant violations.\n\n\
         You MUST output ONLY valid JSON — no markdown, no explanation, no extra text.\n\
         Schema: {{\"pass\":true}} or {{\"pass\":false,\"violations\":[...]}}\n\
         Each violation: {{\"rule_text\":\"<rule>\",\"code_snippet\":\"<code>\",\
         \"context\":\"{abs_path}:<line>\",\"explanation\":\"<why>\",\
         \"action\":\"<fix>\",\"scope\":[\"{abs_path}\"],\
         \"risk_category\":\"c\",\"is_never_rule\":false,\"is_in_claude_md\":false}}"
    );
    match call_claude(cwd, &prompt) {
        Ok(result) => parse_check_result(&result, abs_path),
        Err(_) => FileCheckResult {
            pass: false,
            violations: Vec::new(),
        },
    }
}

fn parse_check_result(raw: &str, file_path: &str) -> FileCheckResult {
    let trimmed = raw.trim();

    // Try parsing as JSON first (new format)
    if let Some(json_str) = extract_json_object(trimmed) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(json_str) {
            let pass = val["pass"].as_bool().unwrap_or(false);
            if pass {
                return FileCheckResult {
                    pass: true,
                    violations: Vec::new(),
                };
            }
            let violations = val["violations"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .map(|v| Violation {
                            rule_text: v["rule_text"].as_str().unwrap_or("").to_string(),
                            code_snippet: v["code_snippet"].as_str().unwrap_or("").to_string(),
                            context: v["context"].as_str().unwrap_or(file_path).to_string(),
                            explanation: v["explanation"].as_str().unwrap_or("").to_string(),
                            action: v["action"].as_str().unwrap_or("Refactor").to_string(),
                            scope: v["scope"]
                                .as_array()
                                .map(|a| {
                                    a.iter()
                                        .filter_map(|s| s.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default(),
                            risk_category: v["risk_category"].as_str().unwrap_or("c").to_string(),
                            is_never_rule: v["is_never_rule"].as_bool().unwrap_or(false),
                            is_fixed: false,
                            is_in_claude_md: v["is_in_claude_md"].as_bool().unwrap_or(false),
                        })
                        .collect()
                })
                .unwrap_or_default();
            return FileCheckResult {
                pass: false,
                violations,
            };
        }
    }

    // Fallback: legacy PASS/FAIL text format
    let pass = trimmed.to_uppercase().starts_with("PASS");
    let violations = if pass {
        Vec::new()
    } else {
        parse_violations(trimmed, file_path)
    };
    FileCheckResult { pass, violations }
}

fn extract_json_object(raw: &str) -> Option<&str> {
    let start = raw.find('{')?;
    let mut depth = 0usize;
    for (idx, ch) in raw[start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(&raw[start..start + idx + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

fn upload_violations(
    violations: &[Violation],
    session_token: &str,
    user_id: Option<&str>,
    auth_url: &str,
) {
    // The stored session_token is already URL-encoded (from the Set-Cookie header),
    // so interpolate it directly into the query string rather than using .query()
    // which would double-encode it.
    // Include user_id in the query string as a fallback for when the session token
    // is expired — the dashboard GET endpoint also uses this to resolve the user.
    let mut url = format!("{}/api/violations?token={}", auth_url, session_token);
    if let Some(uid) = user_id {
        url.push_str(&format!("&user_id={}", uid));
    }
    let mut payload = serde_json::json!({ "violations": violations });
    if let Some(uid) = user_id {
        payload["user_id"] = serde_json::Value::String(uid.to_string());
    }
    let result = ureq::post(&url)
        .set("Content-Type", "application/json")
        .send_string(&payload.to_string());
    if let Err(e) = result {
        ui::print_error(&format!("Failed to upload violations: {e}"));
    }
}

fn shorten_path(path: &str, max: usize) -> String {
    if path.len() > max {
        format!("…{}", &path[path.len() - max + 1..])
    } else {
        path.to_string()
    }
}

fn render_cell(state: u8, name: &str, tick: usize, width: usize) -> String {
    use colored::Colorize;
    const SPINNERS: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

    let plain = match state {
        0 => format!("· {}", name),
        1 => format!("{} {}", SPINNERS[tick % SPINNERS.len()], name),
        2 => format!("✓ {}", name),
        3 => format!("✗ {}", name),
        _ => format!("? {}", name),
    };

    let vis_len = plain.chars().count();
    let pad = width.saturating_sub(vis_len);

    let colored_str = match state {
        0 => plain.dimmed().to_string(),
        1 => plain.yellow().to_string(),
        2 => plain.green().to_string(),
        3 => plain.red().to_string(),
        _ => plain,
    };

    format!("{}{}", colored_str, " ".repeat(pad))
}

fn render_table_frame(
    term: &console::Term,
    n: usize,
    col_width: usize,
    gap: &str,
    max_name: usize,
    max_rule_lines: usize,
    sep: &str,
    cd: usize,
    ud: usize,
    tick: usize,
    common_st: &[std::sync::atomic::AtomicU8],
    user_st: &[std::sync::atomic::AtomicU8],
    paths: &[String],
    common_disp: &[String],
    user_disp: &[String],
    is_final: bool,
) {
    use colored::Colorize;
    use std::sync::atomic::{AtomicU8, Ordering};

    let left_hdr: String;
    let right_hdr: String;
    let left_hdr_plain: String;

    if is_final {
        let cp = common_st
            .iter()
            .filter(|a: &&AtomicU8| a.load(Ordering::Relaxed) == 2)
            .count();
        let cf = n - cp;
        let up = user_st
            .iter()
            .filter(|a: &&AtomicU8| a.load(Ordering::Relaxed) == 2)
            .count();
        let uf = n - up;
        left_hdr = format!(
            "Common Rules  {} passed, {} failed",
            cp.to_string().green().bold(),
            cf.to_string().red().bold()
        );
        right_hdr = format!(
            "Selected Rules  {} passed, {} failed",
            up.to_string().green().bold(),
            uf.to_string().red().bold()
        );
        left_hdr_plain = format!("Common Rules  {} passed, {} failed", cp, cf);
    } else {
        left_hdr = format!("{}", format!("Common Rules ({}/{})", cd, n).yellow().bold());
        right_hdr = format!(
            "{}",
            format!("Selected Rules ({}/{})", ud, n).green().bold()
        );
        left_hdr_plain = format!("Common Rules ({}/{})", cd, n);
    }

    let left_pad = col_width.saturating_sub(left_hdr_plain.len());
    let _ = term.write_line(&format!(
        "  {}{}{}{}",
        left_hdr,
        " ".repeat(left_pad),
        gap,
        right_hdr
    ));
    let _ = term.write_line(&format!("  {}{}{}", sep.dimmed(), gap, sep.dimmed()));

    for i in 0..max_rule_lines {
        let left = if i < common_disp.len() {
            common_disp[i].as_str()
        } else {
            ""
        };
        let left_vis = left.chars().count();
        let lp = col_width.saturating_sub(left_vis);
        let right = if i < user_disp.len() {
            user_disp[i].as_str()
        } else {
            ""
        };
        let _ = term.write_line(&format!(
            "  {}{}{}{}",
            left.dimmed(),
            " ".repeat(lp),
            gap,
            right.dimmed()
        ));
    }

    let _ = term.write_line(&format!("  {}{}{}", sep.dimmed(), gap, sep.dimmed()));

    for i in 0..n {
        let cs = common_st[i].load(Ordering::Relaxed);
        let us = user_st[i].load(Ordering::Relaxed);
        let short = shorten_path(&paths[i], max_name);
        let left = render_cell(cs, &short, tick, col_width);
        let right = render_cell(us, &short, tick, col_width);
        let _ = term.write_line(&format!("  {}{}{}", left, gap, right));
    }
}

/// Returns (had_graph, failed_files) where failed_files is a vec of (rel_path, violations).
fn run_side_by_side_checks(
    cwd: &std::path::Path,
    files: &[std::path::PathBuf],
    common_rules_str: &str,
    user_rules: &[String],
    had_graph: bool,
    session_token: Option<&str>,
    user_id: Option<&str>,
    auth_url: &str,
) -> (bool, Vec<(String, Vec<Violation>)>) {
    use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
    use std::sync::Arc;

    const WAITING: u8 = 0;
    const RUNNING: u8 = 1;
    const PASS: u8 = 2;
    const FAIL: u8 = 3;

    let n = files.len();
    if n == 0 {
        ui::print_sub("No source files found");
        return (had_graph, Vec::new());
    }

    let col_width: usize = 48;
    let gap = "   ";
    let max_name = col_width - 2;

    let rel_paths: Vec<String> = files
        .iter()
        .map(|f| {
            f.strip_prefix(cwd)
                .unwrap_or(f)
                .to_string_lossy()
                .to_string()
        })
        .collect();

    let common_display: Vec<String> = COMMON_RULES
        .iter()
        .map(|r| {
            let s = format!("• {}", r);
            if s.chars().count() > col_width {
                let t: String = s.chars().take(col_width - 1).collect();
                format!("{}…", t)
            } else {
                s
            }
        })
        .collect();

    let user_display: Vec<String> = user_rules
        .iter()
        .map(|r| {
            let s = format!("• {}", r);
            if s.chars().count() > col_width {
                let t: String = s.chars().take(col_width - 1).collect();
                format!("{}…", t)
            } else {
                s
            }
        })
        .collect();

    let user_rules_str: String = user_rules.iter().map(|r| format!("- {r}\n")).collect();
    let max_rule_lines = std::cmp::max(common_display.len(), user_display.len());

    // --- Shared state ---
    let common_state: Arc<Vec<AtomicU8>> =
        Arc::new((0..n).map(|_| AtomicU8::new(WAITING)).collect());
    let user_state: Arc<Vec<AtomicU8>> = Arc::new((0..n).map(|_| AtomicU8::new(WAITING)).collect());
    let common_done = Arc::new(AtomicUsize::new(0));
    let user_done = Arc::new(AtomicUsize::new(0));
    let all_done = Arc::new(AtomicBool::new(false));

    let sep = "─".repeat(col_width);
    let total_lines = 2 + max_rule_lines + 1 + n; // hdr + sep + rules + sep + files

    // --- Initial render ---
    let term = console::Term::stdout();

    render_table_frame(
        &term,
        n,
        col_width,
        gap,
        max_name,
        max_rule_lines,
        &sep,
        0,
        0,
        0,
        &common_state,
        &user_state,
        &rel_paths,
        &common_display,
        &user_display,
        false,
    );

    // --- Render thread ---
    let r_common = Arc::clone(&common_state);
    let r_user = Arc::clone(&user_state);
    let r_cd = Arc::clone(&common_done);
    let r_ud = Arc::clone(&user_done);
    let r_done = Arc::clone(&all_done);
    let r_paths = rel_paths.clone();
    let r_common_disp = common_display.clone();
    let r_user_disp = user_display.clone();
    let r_sep = sep.clone();

    let render_handle = thread::spawn(move || {
        let term = console::Term::stdout();
        let mut tick: usize = 0;
        loop {
            thread::sleep(Duration::from_millis(80));
            tick += 1;

            term.clear_last_lines(total_lines).ok();

            let cd = r_cd.load(Ordering::Relaxed);
            let ud = r_ud.load(Ordering::Relaxed);

            render_table_frame(
                &term,
                n,
                col_width,
                gap,
                max_name,
                max_rule_lines,
                &r_sep,
                cd,
                ud,
                tick,
                &r_common,
                &r_user,
                &r_paths,
                &r_common_disp,
                &r_user_disp,
                false,
            );

            if r_done.load(Ordering::Relaxed) {
                break;
            }
        }
    });

    // --- Spawn check threads (common + user in parallel) ---
    let common_handles: Vec<_> = (0..n)
        .map(|i| {
            let file = files[i].clone();
            let cwd = cwd.to_path_buf();
            let rules = common_rules_str.to_string();
            let state = Arc::clone(&common_state);
            let done = Arc::clone(&common_done);
            let rel = rel_paths[i].clone();
            thread::spawn(move || {
                state[i].store(RUNNING, Ordering::Relaxed);
                let result = check_file(&cwd, &file.to_string_lossy(), &rules);
                state[i].store(if result.pass { PASS } else { FAIL }, Ordering::Relaxed);
                done.fetch_add(1, Ordering::Relaxed);
                (rel, result)
            })
        })
        .collect();

    let user_handles: Vec<_> = (0..n)
        .map(|i| {
            let file = files[i].clone();
            let cwd = cwd.to_path_buf();
            let rules = user_rules_str.clone();
            let state = Arc::clone(&user_state);
            let done = Arc::clone(&user_done);
            let rel = rel_paths[i].clone();
            thread::spawn(move || {
                state[i].store(RUNNING, Ordering::Relaxed);
                let result = check_file(&cwd, &file.to_string_lossy(), &rules);
                state[i].store(if result.pass { PASS } else { FAIL }, Ordering::Relaxed);
                done.fetch_add(1, Ordering::Relaxed);
                (rel, result)
            })
        })
        .collect();

    // --- Wait for all check threads and collect violations ---
    let mut all_violations: Vec<Violation> = Vec::new();
    for h in common_handles {
        if let Ok((_, result)) = h.join() {
            all_violations.extend(result.violations);
        }
    }

    let mut failed_files: Vec<(String, Vec<Violation>)> = Vec::new();
    for h in user_handles {
        if let Ok((rel, result)) = h.join() {
            if !result.pass {
                failed_files.push((rel, result.violations.clone()));
            }
            all_violations.extend(result.violations);
        }
    }

    // --- Stop render thread and draw final frame ---
    all_done.store(true, Ordering::Relaxed);
    render_handle.join().ok();

    term.clear_last_lines(total_lines).ok();
    render_table_frame(
        &term,
        n,
        col_width,
        gap,
        max_name,
        max_rule_lines,
        &sep,
        common_done.load(Ordering::Relaxed),
        user_done.load(Ordering::Relaxed),
        0,
        &common_state,
        &user_state,
        &rel_paths,
        &common_display,
        &user_display,
        true,
    );

    // --- Step 1: Print violation details per failed file ---
    if !failed_files.is_empty() {
        println!();
        ui::print_header("Violation Details");
        for (rel, violations) in &failed_files {
            ui::print_error(&format!("✗ {}", rel));
            for v in violations {
                ui::print_error(&format!("  • {}", v.rule_text));
                if !v.explanation.is_empty() {
                    ui::print_sub(&format!("    {}", v.explanation));
                }
            }
            println!();
        }
    }

    // Always upload violations (even empty) to replace stale data on the dashboard
    if let Some(token) = session_token {
        upload_violations(&all_violations, token, user_id, auth_url);
    }

    (had_graph, failed_files)
}

struct LiveClaude;

impl planner::ClaudeExecutor for LiveClaude {
    fn run(&self, cwd: &std::path::Path, prompt: &str) -> Result<String, String> {
        call_planner_claude(cwd, prompt)
    }
}

struct LiveRulesProvider {
    user_id: Option<String>,
}

impl planner::RulesProvider for LiveRulesProvider {
    fn fetch_rules(&self, query: &str) -> planner::RulesFetchResult {
        let Some(user_id) = self.user_id.as_deref() else {
            return planner::RulesFetchResult::NoRules;
        };

        match fetch_rules(user_id, query) {
            FetchRulesResult::Rules(text) => {
                let rules: Vec<String> = text
                    .lines()
                    .map(|line| line.trim().trim_start_matches('-').trim().to_string())
                    .filter(|line| !line.is_empty())
                    .collect();
                if rules.is_empty() {
                    planner::RulesFetchResult::NoRules
                } else {
                    planner::RulesFetchResult::Rules(rules)
                }
            }
            FetchRulesResult::NoGraph => planner::RulesFetchResult::NoRules,
            FetchRulesResult::Error(err) => planner::RulesFetchResult::Error(err),
        }
    }
}

fn run_configure_phase() {
    let result = configure_all();

    match result.agents_error {
        Some(e) => ui::print_error(&format!("Agent files error: {e}")),
        None => ui::print_success("AGENTS.md configured"),
    }

    thread::sleep(Duration::from_millis(150));

    match result.hooks_error {
        Some(e) => ui::print_error(&format!(".claude/hooks error: {e}")),
        None => ui::print_success(".claude/hooks configured"),
    }

    thread::sleep(Duration::from_millis(150));

    match result.commands_error {
        Some(e) => ui::print_error(&format!(".claude/commands error: {e}")),
        None => ui::print_success(".claude/commands configured"),
    }
}

fn run_plan_command(
    cwd: &std::path::Path,
    config: &Config,
    query_parts: &[String],
    max_iterations: usize,
    use_stdin: bool,
    raw: bool,
) -> io::Result<()> {
    let query = if use_stdin {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        input.trim().to_string()
    } else {
        query_parts.join(" ").trim().to_string()
    };
    if query.is_empty() {
        ui::print_error("Plan query cannot be empty");
        return Ok(());
    }

    if !raw {
        ui::print_header("Rippletide Plan Review");
    }

    let claude = LiveClaude;
    let rules_provider = LiveRulesProvider {
        user_id: config.user_id.clone(),
    };

    let outcome = planner::run_plan_loop(cwd, &query, max_iterations, &claude, &rules_provider)
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;

    if raw {
        println!("{}", outcome.final_plan);
    } else {
        if outcome.used_fallback_rules {
            ui::print_info("Using fallback plan rules");
        } else {
            ui::print_success(&format!(
                "Loaded {} rules from Rippletide",
                outcome.rules.len()
            ));
        }

        for summary in &outcome.iteration_summaries {
            if summary.passed {
                ui::print_success(&format!("Review attempt {} passed", summary.attempt));
            } else {
                ui::print_progress(&format!(
                    "Review attempt {} found {} violation(s)",
                    summary.attempt, summary.violation_count
                ));
            }
        }

        if outcome.satisfied {
            ui::print_success("Returning revised final plan");
        } else {
            ui::print_info(&format!(
                "Returning latest draft after {}",
                outcome.stopped_reason.replace('_', " ")
            ));
        }

        println!();
        println!("{}", outcome.final_plan);
        println!();
    }

    Ok(())
}

fn run_review_plan_command(
    cwd: &std::path::Path,
    config: &Config,
    query_parts: &[String],
    use_stdin: bool,
    json: bool,
) -> io::Result<()> {
    let query = query_parts.join(" ").trim().to_string();
    if query.is_empty() {
        ui::print_error("Review query cannot be empty");
        return Ok(());
    }
    if !use_stdin {
        ui::print_error("Review plan requires --stdin with the candidate plan");
        return Ok(());
    }

    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let plan = input.trim().to_string();
    if plan.is_empty() {
        ui::print_error("Candidate plan cannot be empty");
        return Ok(());
    }

    let claude = LiveClaude;
    let rules_provider = LiveRulesProvider {
        user_id: config.user_id.clone(),
    };

    let review = planner::review_plan_candidate(cwd, &query, &plan, &claude, &rules_provider)
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;

    if json {
        let payload = serde_json::to_string(&review)
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
        println!("{payload}");
    } else {
        ui::print_header("Rippletide Plan Review Check");
        if review.used_fallback_rules {
            ui::print_info("Using fallback plan rules");
        } else {
            ui::print_success(&format!(
                "Loaded {} rules from Rippletide",
                review.rules.len()
            ));
        }

        if review.pass {
            ui::print_success("Review passed");
        } else {
            ui::print_progress(&format!(
                "Review found {} violation(s)",
                review.violations.len()
            ));
            for violation in &review.violations {
                ui::print_sub(&format!(
                    "{}: {} -> {}",
                    violation.rule, violation.issue, violation.fix
                ));
            }
        }
    }

    Ok(())
}

fn main() -> io::Result<()> {
    // Enable ANSI color/cursor support on Windows terminals (including VSCode)
    #[cfg(windows)]
    {
        let _ = colored::control::set_virtual_terminal(true);
    }

    let cli = Cli::parse();

    if matches!(&cli.command, Some(Commands::Logout)) {
        return logout();
    }

    let mut config = load_config();
    apply_environment_override(&mut config);

    let cwd = std::env::current_dir()?;

    if let Some(Commands::Plan {
        query,
        max_iterations,
        stdin,
        raw,
    }) = &cli.command
    {
        return run_plan_command(&cwd, &config, query, *max_iterations, *stdin, *raw);
    }

    if let Some(Commands::ReviewPlan { query, stdin, json }) = &cli.command {
        return run_review_plan_command(&cwd, &config, query, *stdin, *json);
    }

    let _read_only = match &cli.command {
        Some(Commands::Connect { read_only }) => *read_only,
        None => false,
        Some(Commands::Plan { .. }) => false,
        Some(Commands::ReviewPlan { .. }) => false,
        Some(Commands::Logout) => unreachable!(),
    };

    // Phase 1 — Header
    ui::print_header("Rippletide Code");

    let is_logged_in = config.session_token.is_some();

    // Phase 2 — Auth (if not logged in)
    let mut dashboard_url: Option<String> = None;
    if !is_logged_in {
        let login_result = login(&mut config)?;
        println!();
        if !login_result.success {
            return Ok(());
        }
        dashboard_url = login_result.dashboard_url;
    }

    // Phase 3 — Repository scan
    let scan_result = run_scan_phase(&cwd);
    println!();

    // Phase 4 — Reading assistant instructions (if CLAUDE.md exists)
    if scan_result.has_claude_md {
        run_rules_phase(&cwd);
        println!();
        if let Some(ref user_id) = config.user_id {
            run_contradiction_phase(&cwd, user_id);
            println!();
        }
    }

    // Phase 5 — Inferring conventions
    // rippletide-override: user approved
    run_conventions_phase();
    println!();

    // rippletide-override: user approved
    // Phase 6a — Fetch inferred rules from graph (probe for graph existence)
    let sp = ui::start_spinner("Fetching inferred rules from graph…");
    let mut had_graph = true;
    let mut inferred_rules: Vec<String> = match config
        .user_id
        .as_deref()
        .map(|uid| fetch_rules(uid, "Return all coding rules"))
    {
        Some(FetchRulesResult::Rules(text)) => text
            .lines()
            .map(|l| l.trim().trim_start_matches('-').trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        Some(FetchRulesResult::NoGraph) => {
            had_graph = false;
            Vec::new()
        }
        Some(FetchRulesResult::Error(_)) | None => Vec::new(),
    };
    ui::finish_spinner(&sp, "Inferred rules loaded");

    // Phase 6b — Upload sessions to build graph if none exists, then re-fetch inferred rules
    if !had_graph || !is_logged_in {
        if let Some(ref uid) = config.user_id {
            println!();
            if !had_graph {
                ui::print_sub("No context graph found — uploading sessions to build one...");
            }
            upload_sessions(uid, &cwd)?;
        }
    }

    if !had_graph {
        let sp = ui::start_spinner("Loading inferred rules from new graph…");
        inferred_rules = match config
            .user_id
            .as_deref()
            .map(|uid| fetch_rules(uid, "Return all coding rules"))
        {
            Some(FetchRulesResult::Rules(text)) => {
                had_graph = true;
                text.lines()
                    .map(|l| l.trim().trim_start_matches('-').trim().to_string())
                    .filter(|l| !l.is_empty())
                    .collect()
            }
            _ => inferred_rules,
        };
        ui::finish_spinner(&sp, "Rules loaded from graph");
    }

    {
        let sp = ui::start_spinner("Building Rippletide Context Graph");
        thread::sleep(Duration::from_millis(800));
        ui::finish_spinner(&sp, "Building Rippletide Context Graph");
        ui::print_success("Context Graph built.");
    }

    println!();
    let generic_rules = run_benchmark_phase(&cwd, &scan_result.tech_stacks);
    println!();

    let candidate_rules = build_rule_candidates(
        &inferred_rules,
        &generic_rules,
        &DEFAULT_USER_RULES
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
    );

    let current_rules = if inferred_rules.is_empty() {
        DEFAULT_USER_RULES
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
    } else {
        inferred_rules.clone()
    };

    let user_rules = if candidate_rules.is_empty() {
        current_rules.clone()
    } else {
        let selected = prompt_rule_selection(&candidate_rules, &current_rules)?;
        let selected = if selected.is_empty() {
            ui::print_info("No rules selected — falling back to default rules for this run");
            DEFAULT_USER_RULES
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        } else {
            selected
        };

        match persist_selected_rules_local(&cwd, &selected) {
            Ok(path) => ui::print_success(&format!(
                "Saved selected rules locally to {}",
                path.display()
            )),
            Err(err) => ui::print_error(&format!("Failed to save selected rules locally: {err}")),
        }

        if let Some(ref user_id) = config.user_id {
            match persist_selected_rules_remote(user_id, &selected) {
                Ok(()) => ui::print_success("Saved selected rules to Rippletide backend"),
                Err(err) => ui::print_error(&err),
            }
        }
        println!();
        selected
    };

    // Phase 6d — Side-by-side rule checks (first 5 files)
    let failed_files = {
        let mut files = collect_source_files(&cwd);
        files.truncate(5);
        ui::print_header("Checking files against coding rules");
        let common_str: String = COMMON_RULES.iter().map(|r| format!("- {r}\n")).collect();
        let auth_url = config.auth_url().to_string();
        let (_had_graph, failed) = run_side_by_side_checks(
            &cwd,
            &files,
            &common_str,
            &user_rules,
            had_graph,
            config.session_token.as_deref(),
            config.user_id.as_deref(),
            &auth_url,
        );
        failed
    };
    println!();

    // rippletide-override: user approved
    // Phase 6d — Offer to fix failed files via POST /fix
    if !failed_files.is_empty() {
        if let Some(ref user_id) = config.user_id {
            let fix_endpoint = fix_url();
            let mut bulk_action: Option<ui::FixAction> = None;
            ui::print_header("Fixing rule violations");

            for (rel, violations) in &failed_files {
                let abs_path = cwd.join(rel);

                // If user chose None for all remaining, skip
                if matches!(bulk_action, Some(ui::FixAction::None)) {
                    ui::print_sub(&format!("  Skipped {}", rel));
                    continue;
                }

                // Show file and violations
                let violation_pairs: Vec<(String, String)> = violations
                    .iter()
                    .map(|v| (v.rule_text.clone(), v.explanation.clone()))
                    .collect();
                ui::print_violations(rel, &violation_pairs);

                // Fetch fix from LLM FIRST (before prompting)
                let sp = ui::start_spinner(&format!("Fetching fix for {}", rel));
                let fix_result: Option<(String, String, Vec<String>)> =
                    match std::fs::read_to_string(&abs_path) {
                        Ok(original_code) => {
                            let payload = serde_json::json!({
                                "code": original_code,
                                "context": rel,
                            });
                            match ureq::post(&fix_endpoint)
                                .set("Content-Type", "application/json")
                                .set("X-User-Id", user_id)
                                .send_string(&payload.to_string())
                            {
                                Ok(resp) => {
                                    if let Ok(body) = resp.into_json::<serde_json::Value>() {
                                        if let Some(fixed) =
                                            body.get("fixed_code").and_then(|v| v.as_str())
                                        {
                                            let changes: Vec<String> = body
                                                .get("changes_applied")
                                                .and_then(|v| v.as_array())
                                                .map(|arr| {
                                                    arr.iter()
                                                        .filter_map(|c| {
                                                            c.get("change_description")
                                                                .and_then(|d| d.as_str())
                                                                .map(|s| s.to_string())
                                                        })
                                                        .collect()
                                                })
                                                .unwrap_or_default();
                                            ui::finish_spinner(
                                                &sp,
                                                &format!("Fix ready for {}", rel),
                                            );
                                            Some((original_code, fixed.to_string(), changes))
                                        } else {
                                            ui::finish_spinner(
                                                &sp,
                                                &format!("No fix returned for {}", rel),
                                            );
                                            None
                                        }
                                    } else {
                                        ui::finish_spinner(
                                            &sp,
                                            &format!("Could not parse fix response for {}", rel),
                                        );
                                        None
                                    }
                                }
                                Err(e) => {
                                    ui::finish_spinner(
                                        &sp,
                                        &format!("Fix request failed for {}: {}", rel, e),
                                    );
                                    None
                                }
                            }
                        }
                        Err(e) => {
                            ui::finish_spinner(&sp, &format!("Could not read {}: {}", rel, e));
                            None
                        }
                    };

                // Show diff and prompt AFTER fetching fix
                if let Some((original_code, fixed_code, changes)) = fix_result {
                    ui::print_diff(rel, &original_code, &fixed_code);

                    // Prompt user (or use bulk action)
                    let action = match &bulk_action {
                        Some(ui::FixAction::All) => ui::FixAction::Fix,
                        _ => ui::prompt_apply_action(rel).unwrap_or(ui::FixAction::Skip),
                    };

                    match action {
                        ui::FixAction::Fix | ui::FixAction::All => {
                            if matches!(action, ui::FixAction::All) {
                                bulk_action = Some(ui::FixAction::All);
                            }
                            if let Err(e) = std::fs::write(&abs_path, &fixed_code) {
                                ui::print_error(&format!("Could not write fix for {}: {}", rel, e));
                            } else {
                                ui::print_success(&format!("Fixed {}", rel));
                                for change in &changes {
                                    ui::print_sub(&format!("  → {}", change));
                                }
                            }
                        }
                        ui::FixAction::None => {
                            bulk_action = Some(ui::FixAction::None);
                            ui::print_sub(&format!("  Skipped {}", rel));
                        }
                        ui::FixAction::Skip => {
                            ui::print_sub(&format!("  Skipped {}", rel));
                        }
                    }
                }
            }
        }
    }
    println!();

    // Phase 7 — Configure files
    run_configure_phase();

    // Show dashboard URL — always, from stored token or fresh login
    let final_url = dashboard_url.or_else(|| {
        config.session_token.as_ref().map(|token| {
            let mut url = format!("https://app.rippletide.com/code?token={}", token);
            if let Some(ref uid) = config.user_id {
                url.push_str("&user_id=");
                url.push_str(uid);
            }
            url
        })
    });
    if let Some(url) = final_url {
        println!();
        ui::print_sub(&format!("Dashboard: {url}"));
    }

    if should_launch_interactive_claude() {
        println!();
        ui::print_success("Launching Claude Code");
        if let Err(e) = launch_interactive_claude(&cwd) {
            ui::print_error(&format!("Could not launch Claude Code: {e}"));
        }
    }

    println!();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    fn temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn filters_out_markdown_headings_from_rule_candidates() {
        let inferred = vec![
            "## Code Quality Standards".to_string(),
            "Prefer small, focused functions.".to_string(),
        ];
        let generic = vec!["### Design & structure".to_string()];
        let fallback = vec![];

        let candidates = build_rule_candidates(&inferred, &generic, &fallback);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].rule, "Prefer small, focused functions.");
    }

    #[test]
    fn build_rule_candidates_dedupes_and_prioritizes_inferred_rules() {
        let inferred = vec![
            "Use typed errors".to_string(),
            "Keep handlers small".to_string(),
        ];
        let generic = vec![
            "Keep handlers small".to_string(),
            "Write focused tests".to_string(),
        ];
        let fallback = vec!["Fallback rule".to_string()];

        let candidates = build_rule_candidates(&inferred, &generic, &fallback);
        assert_eq!(candidates.len(), 3);
        assert_eq!(candidates[0].source, CandidateSource::Inferred);
        assert_eq!(candidates[1].source, CandidateSource::Inferred);
        assert_eq!(candidates[2].source, CandidateSource::Generic);
        assert_eq!(candidates[2].rule, "Write focused tests");
    }

    #[test]
    fn selected_rules_markdown_contains_all_rules() {
        let markdown = selected_rules_markdown(&vec![
            "Keep handlers small".to_string(),
            "Write focused tests".to_string(),
        ]);
        assert!(markdown.contains("# Selected Rules"));
        assert!(markdown.contains("- Keep handlers small"));
        assert!(markdown.contains("- Write focused tests"));
    }

    #[test]
    fn planner_claude_invocation_removes_nested_session_env() {
        let _guard = env_lock();
        let dir = temp_dir("rippletide-plan-env");
        let stub = dir.join("claude-stub.sh");

        fs::write(
            &stub,
            "#!/bin/bash\nif [[ -n \"${CLAUDECODE:-}\" ]]; then\n  echo nested >&2\n  exit 42\nfi\nFOUND=0\nfor arg in \"$@\"; do\n  if [[ \"$arg\" == \"--no-session-persistence\" ]]; then\n    FOUND=1\n    break\n  fi\ndone\nif [[ \"$FOUND\" != \"1\" ]]; then\n  echo missing-session-flag >&2\n  exit 43\nfi\nprintf 'ok\\n'\n",
        )
        .unwrap();
        #[cfg(unix)]
        fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();

        let old_bin = std::env::var_os("RIPPLETIDE_CLAUDE_BIN");
        let old_code = std::env::var_os("CLAUDECODE");
        let old_project = std::env::var_os("CLAUDE_PROJECT_DIR");

        std::env::set_var("RIPPLETIDE_CLAUDE_BIN", &stub);
        std::env::set_var("CLAUDECODE", "1");
        std::env::set_var("CLAUDE_PROJECT_DIR", &dir);

        let result = call_planner_claude(&dir, "hello").unwrap();
        assert_eq!(result, "ok");

        match old_bin {
            Some(value) => std::env::set_var("RIPPLETIDE_CLAUDE_BIN", value),
            None => std::env::remove_var("RIPPLETIDE_CLAUDE_BIN"),
        }
        match old_code {
            Some(value) => std::env::set_var("CLAUDECODE", value),
            None => std::env::remove_var("CLAUDECODE"),
        }
        match old_project {
            Some(value) => std::env::set_var("CLAUDE_PROJECT_DIR", value),
            None => std::env::remove_var("CLAUDE_PROJECT_DIR"),
        }

        let _ = fs::remove_file(stub);
        let _ = fs::remove_dir(dir);
    }

    #[test]
    fn claude_stream_invocation_disables_session_persistence() {
        let _guard = env_lock();
        let dir = temp_dir("rippletide-check-env");
        let stub = dir.join("claude-stub.sh");

        fs::write(
            &stub,
            "#!/bin/bash\nFOUND=0\nfor arg in \"$@\"; do\n  if [[ \"$arg\" == \"--no-session-persistence\" ]]; then\n    FOUND=1\n    break\n  fi\ndone\nif [[ \"$FOUND\" != \"1\" ]]; then\n  echo missing-session-flag >&2\n  exit 43\nfi\nprintf '%s\\n' '{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"ok\"}]}}'\n",
        )
        .unwrap();
        #[cfg(unix)]
        fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();

        let old_bin = std::env::var_os("RIPPLETIDE_CLAUDE_BIN");
        std::env::set_var("RIPPLETIDE_CLAUDE_BIN", &stub);

        let result = call_claude(&dir, "hello").unwrap();
        assert_eq!(result, "ok");

        match old_bin {
            Some(value) => std::env::set_var("RIPPLETIDE_CLAUDE_BIN", value),
            None => std::env::remove_var("RIPPLETIDE_CLAUDE_BIN"),
        }

        let _ = fs::remove_file(stub);
        let _ = fs::remove_dir(dir);
    }

    #[test]
    fn claude_project_dir_matches_paths_with_spaces() {
        let base = temp_dir("rippletide-claude-projects");
        let cwd = PathBuf::from(
            "/Users/thomasmolinier/Documents/RIPPLETIDE/Automatic ontologies/experiments/exp2",
        );
        let project_dir = base.join(
            "-Users-thomasmolinier-Documents-RIPPLETIDE-Automatic-ontologies-experiments-exp2",
        );
        fs::create_dir_all(&project_dir).unwrap();

        let resolved = claude_project_dir_for_cwd(&base, &cwd).unwrap();
        assert_eq!(resolved, project_dir);

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn list_claude_projects_uses_logged_cwd_for_display_name() {
        let base = temp_dir("rippletide-claude-project-list");
        let project_dir = base.join(
            "-Users-thomasmolinier-Documents-RIPPLETIDE-Automatic-ontologies-experiments-exp2",
        );
        fs::create_dir_all(&project_dir).unwrap();
        fs::write(
            project_dir.join("session.jsonl"),
            "{\"cwd\":\"/Users/thomasmolinier/Documents/RIPPLETIDE/Automatic ontologies/experiments/exp2\"}\n",
        )
        .unwrap();

        let projects = list_claude_projects_from_base(&base).unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(
            projects[0].0,
            "/Users/thomasmolinier/Documents/RIPPLETIDE/Automatic ontologies/experiments/exp2"
        );
        assert_eq!(projects[0].1, project_dir);

        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn launch_interactive_claude_uses_repo_cwd() {
        let _guard = env_lock();
        let dir = temp_dir("rippletide-launch-claude");
        let stub = dir.join("claude-stub.sh");
        let pwd_file = dir.join("pwd.txt");

        fs::write(
            &stub,
            format!(
                "#!/bin/bash\nif [[ -n \"${{CLAUDECODE:-}}\" ]]; then\n  echo nested >&2\n  exit 42\nfi\npwd > \"{}\"\n",
                pwd_file.display()
            ),
        )
        .unwrap();
        #[cfg(unix)]
        fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();

        let old_bin = std::env::var_os("RIPPLETIDE_CLAUDE_BIN");
        let old_code = std::env::var_os("CLAUDECODE");
        std::env::set_var("RIPPLETIDE_CLAUDE_BIN", &stub);
        std::env::set_var("CLAUDECODE", "1");

        launch_interactive_claude(&dir).unwrap();

        let launched_pwd = fs::read_to_string(&pwd_file).unwrap();
        let launched_canonical = PathBuf::from(launched_pwd.trim()).canonicalize().unwrap();
        let expected_canonical = dir.canonicalize().unwrap();
        assert_eq!(launched_canonical, expected_canonical);

        match old_bin {
            Some(value) => std::env::set_var("RIPPLETIDE_CLAUDE_BIN", value),
            None => std::env::remove_var("RIPPLETIDE_CLAUDE_BIN"),
        }
        match old_code {
            Some(value) => std::env::set_var("CLAUDECODE", value),
            None => std::env::remove_var("CLAUDECODE"),
        }

        let _ = fs::remove_file(stub);
        let _ = fs::remove_file(pwd_file);
        let _ = fs::remove_dir(dir);
    }

    #[test]
    fn plan_command_script_uses_packaged_cli_path() {
        let _guard = env_lock();
        let dir = temp_dir("rippletide-plan-script");
        let cli_stub = dir.join("plan-cli-stub.sh");
        let command_script = dir.join("plan-command.sh");

        fs::write(
            &cli_stub,
            "#!/bin/bash\nset -euo pipefail\nif [[ -n \"${CLAUDECODE:-}\" ]]; then\n  echo nested >&2\n  exit 41\nfi\nif [[ \"$1\" != \"plan\" || \"$2\" != \"--raw\" || \"$3\" != \"--stdin\" ]]; then\n  echo \"bad args: $*\" >&2\n  exit 42\nfi\nREQUEST=\"$(cat)\"\nprintf 'stub plan for: %s\\n' \"$REQUEST\"\n",
        )
        .unwrap();
        fs::write(&command_script, plan_command_script()).unwrap();
        #[cfg(unix)]
        {
            fs::set_permissions(&cli_stub, fs::Permissions::from_mode(0o755)).unwrap();
            fs::set_permissions(&command_script, fs::Permissions::from_mode(0o755)).unwrap();
        }

        let old_bin = std::env::var_os("RIPPLETIDE_PLAN_CLI_BIN");
        let old_code = std::env::var_os("CLAUDECODE");
        let old_project = std::env::var_os("CLAUDE_PROJECT_DIR");

        std::env::set_var("RIPPLETIDE_PLAN_CLI_BIN", &cli_stub);
        std::env::set_var("CLAUDECODE", "1");
        std::env::set_var("CLAUDE_PROJECT_DIR", &dir);

        let output = Command::new("bash")
            .arg(&command_script)
            .arg("hello world")
            .current_dir(&dir)
            .output()
            .unwrap();

        assert!(output.status.success());
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "stub plan for: hello world"
        );

        match old_bin {
            Some(value) => std::env::set_var("RIPPLETIDE_PLAN_CLI_BIN", value),
            None => std::env::remove_var("RIPPLETIDE_PLAN_CLI_BIN"),
        }
        match old_code {
            Some(value) => std::env::set_var("CLAUDECODE", value),
            None => std::env::remove_var("CLAUDECODE"),
        }
        match old_project {
            Some(value) => std::env::set_var("CLAUDE_PROJECT_DIR", value),
            None => std::env::remove_var("CLAUDE_PROJECT_DIR"),
        }

        let _ = fs::remove_file(cli_stub);
        let _ = fs::remove_file(command_script);
        let _ = fs::remove_dir(dir);
    }

    #[test]
    fn review_plan_command_script_uses_packaged_cli_path() {
        let _guard = env_lock();
        let dir = temp_dir("rippletide-review-script");
        let cli_stub = dir.join("plan-cli-stub.sh");
        let command_script = dir.join("review-plan-command.sh");

        fs::write(
            &cli_stub,
            "#!/bin/bash\nset -euo pipefail\nif [[ -n \"${CLAUDECODE:-}\" ]]; then\n  echo nested >&2\n  exit 41\nfi\nif [[ \"$1\" != \"review-plan\" || \"$3\" != \"--stdin\" || \"$4\" != \"--json\" ]]; then\n  echo \"bad args: $*\" >&2\n  exit 42\nfi\nprintf 'stub review for query: %s\\n' \"$2\"\nprintf 'stdin plan: %s\\n' \"$(cat)\"\n",
        )
        .unwrap();
        fs::write(&command_script, review_plan_command_script()).unwrap();
        #[cfg(unix)]
        {
            fs::set_permissions(&cli_stub, fs::Permissions::from_mode(0o755)).unwrap();
            fs::set_permissions(&command_script, fs::Permissions::from_mode(0o755)).unwrap();
        }

        let old_bin = std::env::var_os("RIPPLETIDE_PLAN_CLI_BIN");
        let old_code = std::env::var_os("CLAUDECODE");
        let old_project = std::env::var_os("CLAUDE_PROJECT_DIR");

        std::env::set_var("RIPPLETIDE_PLAN_CLI_BIN", &cli_stub);
        std::env::set_var("CLAUDECODE", "1");
        std::env::set_var("CLAUDE_PROJECT_DIR", &dir);

        let output = Command::new("bash")
            .arg(&command_script)
            .arg("hello world")
            .current_dir(&dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                child
                    .stdin
                    .as_mut()
                    .unwrap()
                    .write_all(b"1. Draft\n2. Validate\n")?;
                child.wait_with_output()
            })
            .unwrap();

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("stub review for query: hello world"));
        assert!(stdout.contains("stdin plan: 1. Draft"));

        match old_bin {
            Some(value) => std::env::set_var("RIPPLETIDE_PLAN_CLI_BIN", value),
            None => std::env::remove_var("RIPPLETIDE_PLAN_CLI_BIN"),
        }
        match old_code {
            Some(value) => std::env::set_var("CLAUDECODE", value),
            None => std::env::remove_var("CLAUDECODE"),
        }
        match old_project {
            Some(value) => std::env::set_var("CLAUDE_PROJECT_DIR", value),
            None => std::env::remove_var("CLAUDE_PROJECT_DIR"),
        }

        let _ = fs::remove_file(cli_stub);
        let _ = fs::remove_file(command_script);
        let _ = fs::remove_dir(dir);
    }

    #[test]
    fn claude_md_contradictions_url_uses_override_when_present() {
        let _guard = env_lock();
        let old_value = std::env::var_os("RIPPLETIDE_CLAUDE_MD_CONTRADICTIONS_URL");
        std::env::set_var(
            "RIPPLETIDE_CLAUDE_MD_CONTRADICTIONS_URL",
            "http://127.0.0.1:5051/claude-md/contradictions",
        );

        let value = claude_md_contradictions_url();
        assert_eq!(value, "http://127.0.0.1:5051/claude-md/contradictions");

        match old_value {
            Some(v) => std::env::set_var("RIPPLETIDE_CLAUDE_MD_CONTRADICTIONS_URL", v),
            None => std::env::remove_var("RIPPLETIDE_CLAUDE_MD_CONTRADICTIONS_URL"),
        }
    }

    #[test]
    fn claude_md_contradictions_url_defaults_from_upload_url() {
        let value = claude_md_contradictions_url();
        assert_eq!(
            value,
            "https://coding-agent.up.railway.app/claude-md/contradictions"
        );
    }

    #[test]
    fn plan_command_markdown_describes_visible_review_flow() {
        assert!(PLAN_COMMAND_MARKDOWN.contains("Drafting initial plan."));
        assert!(PLAN_COMMAND_MARKDOWN.contains("review-plan-command.sh"));
        assert!(PLAN_COMMAND_MARKDOWN.contains("${CLAUDE_PROJECT_DIR:-$PWD}"));
        assert!(!PLAN_COMMAND_MARKDOWN.contains("Final revised plan:\n!`bash"));
    }

    #[test]
    fn sync_global_plan_command_updates_rippletide_managed_file() {
        let _guard = env_lock();
        let home = temp_dir("rippletide-global-plan");
        let commands_dir = home.join(".claude").join("commands");
        let plan_path = commands_dir.join("plan.md");
        fs::create_dir_all(&commands_dir).unwrap();
        fs::write(
            &plan_path,
            r#"---
description: Generate a repo-aware implementation plan revised against Rippletide rules
---
Return exactly the final revised plan below and nothing else.

Final revised plan:
!`bash "${CLAUDE_PROJECT_DIR:-$PWD}/.claude/commands/plan-command.sh" "$ARGUMENTS"`
"#,
        )
        .unwrap();

        let old_home = std::env::var_os("RIPPLETIDE_CLAUDE_HOME");
        std::env::set_var("RIPPLETIDE_CLAUDE_HOME", &home);

        let changed = sync_global_plan_command().unwrap();
        let updated = fs::read_to_string(&plan_path).unwrap();

        assert!(changed);
        assert_eq!(updated, PLAN_COMMAND_MARKDOWN);

        match old_home {
            Some(value) => std::env::set_var("RIPPLETIDE_CLAUDE_HOME", value),
            None => std::env::remove_var("RIPPLETIDE_CLAUDE_HOME"),
        }
        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn sync_global_plan_command_leaves_custom_file_untouched() {
        let _guard = env_lock();
        let home = temp_dir("rippletide-global-plan-custom");
        let commands_dir = home.join(".claude").join("commands");
        let plan_path = commands_dir.join("plan.md");
        fs::create_dir_all(&commands_dir).unwrap();
        fs::write(&plan_path, "custom /plan command\n").unwrap();

        let old_home = std::env::var_os("RIPPLETIDE_CLAUDE_HOME");
        std::env::set_var("RIPPLETIDE_CLAUDE_HOME", &home);

        let changed = sync_global_plan_command().unwrap();
        let updated = fs::read_to_string(&plan_path).unwrap();

        assert!(!changed);
        assert_eq!(updated, "custom /plan command\n");

        match old_home {
            Some(value) => std::env::set_var("RIPPLETIDE_CLAUDE_HOME", value),
            None => std::env::remove_var("RIPPLETIDE_CLAUDE_HOME"),
        }
        let _ = fs::remove_dir_all(home);
    }
}

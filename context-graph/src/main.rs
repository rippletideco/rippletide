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
    /// Select markdown files to share or push
    SelectFiles {
        /// Print the selected file ids as JSON
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
const CODING_AGENT_BASE_URL: &str = "https://coding-agent.up.railway.app";
const UPLOAD_PATH: &str = "/upload";
const CHECK_CODE_PATH: &str = "/check-code";
const REVIEW_PLAN_BLOCKS_PATH: &str = "/review-plan-blocks";
const FILES_PATH: &str = "/files";
const CLAUDE_MD_CONTRADICTIONS_PATH: &str = "/claude-md/contradictions";
#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Environment {
    Local,
    Production,
    Enterprise,
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
            Self::Enterprise => write!(f, "enterprise"),
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
            Environment::Enterprise => PROD_API_URL,
        }
    }

    fn auth_url(&self) -> &str {
        match self.environment {
            Environment::Local => LOCAL_AUTH_URL,
            Environment::Production => PROD_AUTH_URL,
            Environment::Enterprise => PROD_AUTH_URL,
        }
    }

    fn is_connected(&self) -> bool {
        match self.environment {
            Environment::Enterprise => self
                .user_id
                .as_deref()
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false),
            Environment::Local | Environment::Production => self
                .session_token
                .as_deref()
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false),
        }
    }

    fn configured_coding_agent_base_url(&self) -> Option<String> {
        self.api_url
            .as_deref()
            .and_then(normalize_coding_agent_base_url)
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
            "enterprise" => config.environment = Environment::Enterprise,
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

const HOOK_SCRIPT: &str = include_str!("../../.claude/hooks/fetch-rules.sh");
const MANAGE_RULE_SCRIPT: &str = include_str!("../../.claude/hooks/manage-rule.sh");
const INVITE_RULES_SCRIPT: &str = include_str!("../../.claude/hooks/invite-rules.sh");
const RECEIVE_RULES_SCRIPT: &str = include_str!("../../.claude/hooks/receive-rules.sh");
const CHECK_CODE_SCRIPT: &str = include_str!("../../.claude/hooks/check-code.sh");
const CREATE_TEAM_SCRIPT: &str = include_str!("../../.claude/hooks/create-team.sh");
const JOIN_TEAM_SCRIPT: &str = include_str!("../../.claude/hooks/join-team.sh");
const APPROVE_JOIN_SCRIPT: &str = include_str!("../../.claude/hooks/approve-join.sh");
const PUSH_RULES_SCRIPT: &str = include_str!("../../.claude/hooks/push-rules.sh");
const SYNC_RULES_SCRIPT: &str = include_str!("../../.claude/hooks/sync-rules.sh");

// rippletide-override: user approved
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

// rippletide-override: user approved
/// Hook type for settings.json generation.
enum HookType {
    UserPromptSubmit,
    PreToolUse,
}

/// A hook script entry: filename, content, type, timeout, and optional tool matcher.
struct HookEntry {
    name: &'static str,
    content: &'static str,
    hook_type: HookType,
    timeout: u32,
    matcher: Option<&'static str>,
}

/// All hook scripts to install. Add new hooks here — settings.json is generated automatically.
const HOOK_ENTRIES: &[HookEntry] = &[
    HookEntry {
        name: "fetch-rules.sh",
        content: HOOK_SCRIPT,
        hook_type: HookType::UserPromptSubmit,
        timeout: 180,
        matcher: None,
    },
    HookEntry {
        name: "invite-rules.sh",
        content: INVITE_RULES_SCRIPT,
        hook_type: HookType::UserPromptSubmit,
        timeout: 60,
        matcher: None,
    },
    HookEntry {
        name: "receive-rules.sh",
        content: RECEIVE_RULES_SCRIPT,
        hook_type: HookType::UserPromptSubmit,
        timeout: 120,
        matcher: None,
    },
    HookEntry {
        name: "create-team.sh",
        content: CREATE_TEAM_SCRIPT,
        hook_type: HookType::UserPromptSubmit,
        timeout: 60,
        matcher: None,
    },
    HookEntry {
        name: "join-team.sh",
        content: JOIN_TEAM_SCRIPT,
        hook_type: HookType::UserPromptSubmit,
        timeout: 60,
        matcher: None,
    },
    HookEntry {
        name: "approve-join.sh",
        content: APPROVE_JOIN_SCRIPT,
        hook_type: HookType::UserPromptSubmit,
        timeout: 60,
        matcher: None,
    },
    HookEntry {
        name: "push-rules.sh",
        content: PUSH_RULES_SCRIPT,
        hook_type: HookType::UserPromptSubmit,
        timeout: 60,
        matcher: None,
    },
    HookEntry {
        name: "sync-rules.sh",
        content: SYNC_RULES_SCRIPT,
        hook_type: HookType::UserPromptSubmit,
        timeout: 120,
        matcher: None,
    },
    HookEntry {
        name: "check-code.sh",
        content: CHECK_CODE_SCRIPT,
        hook_type: HookType::PreToolUse,
        timeout: 60,
        matcher: Some("Write|Edit|MultiEdit"),
    },
    HookEntry {
        name: "manage-rule.sh",
        content: MANAGE_RULE_SCRIPT,
        hook_type: HookType::UserPromptSubmit,
        timeout: 60,
        matcher: None,
    },
];

/// Build settings.json from HOOK_ENTRIES at runtime.
fn generate_settings_json() -> String {
    let mut user_prompt_submit = Vec::new();
    let mut pre_tool_use = Vec::new();

    for entry in HOOK_ENTRIES {
        let hook_obj = if let Some(m) = entry.matcher {
            format!(
                r#"      {{
        "matcher": "{}",
        "hooks": [
          {{
            "type": "command",
            "command": "bash \"$CLAUDE_PROJECT_DIR/.claude/hooks/{}\"",
            "timeout": {}
          }}
        ]
      }}"#,
                m, entry.name, entry.timeout
            )
        } else {
            format!(
                r#"      {{
        "hooks": [
          {{
            "type": "command",
            "command": "bash \"$CLAUDE_PROJECT_DIR/.claude/hooks/{}\"",
            "timeout": {}
          }}
        ]
      }}"#,
                entry.name, entry.timeout
            )
        };

        match entry.hook_type {
            HookType::UserPromptSubmit => user_prompt_submit.push(hook_obj),
            HookType::PreToolUse => pre_tool_use.push(hook_obj),
        }
    }

    format!(
        "{{\n  \"hooks\": {{\n    \"UserPromptSubmit\": [\n{}\n    ],\n    \"PreToolUse\": [\n{}\n    ]\n  }}\n}}\n",
        user_prompt_submit.join(",\n"),
        pre_tool_use.join(",\n"),
    )
}

fn ensure_claude_hooks() -> io::Result<bool> {
    let cwd = std::env::current_dir()?;
    let hooks_dir = cwd.join(".claude").join("hooks");
    let settings_path = cwd.join(".claude").join("settings.json");
    let local_settings_path = cwd.join(".claude").join("settings.local.json");

    fs::create_dir_all(&hooks_dir)?;

    let mut changed = false;

    // Install all hook scripts from HOOK_ENTRIES
    for entry in HOOK_ENTRIES {
        let path = hooks_dir.join(entry.name);
        let needs_write = if path.exists() {
            fs::read_to_string(&path)? != entry.content
        } else {
            true
        };
        if needs_write {
            fs::write(&path, entry.content)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o755))?;
            }
            changed = true;
        }
    }

    // Generate settings.json from HOOK_ENTRIES
    let settings_content = generate_settings_json();
    let needs_settings = if settings_path.exists() {
        fs::read_to_string(&settings_path)? != settings_content
    } else {
        true
    };
    if needs_settings {
        fs::write(&settings_path, &settings_content)?;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SetupModeChoice {
    Individual,
    Enterprise,
}

fn should_prompt_setup_mode(config: &Config) -> bool {
    !config.is_connected() || custom_coding_agent_backend_override().is_some()
}

fn parse_setup_mode_choice(input: &str) -> Option<SetupModeChoice> {
    match input.trim().to_lowercase().as_str() {
        "" | "1" | "i" | "individual" => Some(SetupModeChoice::Individual),
        "2" | "e" | "enterprise" => Some(SetupModeChoice::Enterprise),
        _ => None,
    }
}

fn prompt_enterprise_backend_url() -> io::Result<Option<String>> {
    let backend_url = ui::styled_prompt_for_mode(
        ui::ConnectionMode::Enterprise,
        "Enter enterprise backend URL: ",
    )?;
    let Some(normalized) = normalize_coding_agent_base_url(&backend_url) else {
        ui::print_error("Enterprise backend URL is invalid");
        return Ok(None);
    };
    Ok(Some(normalized))
}

fn select_setup_mode(config: &mut Config) -> io::Result<Option<LoginResult>> {
    if !should_prompt_setup_mode(config) {
        return Ok(None);
    }

    let custom_backend = custom_coding_agent_backend_override();
    println!();
    ui::print_result("Choose connection mode");
    if let Some(ref backend_url) = custom_backend {
        ui::print_sub(&format!("Custom backend detected: {backend_url}"));
    }
    ui::print_sub("1. Individual workspace");
    ui::print_sub("2. Enterprise backend");
    println!();

    let choice = loop {
        let raw = ui::styled_prompt("Select mode [1/2]: ")?;
        if let Some(choice) = parse_setup_mode_choice(&raw) {
            break choice;
        }
        ui::print_error("Choose 1 for individual or 2 for enterprise");
    };

    match choice {
        SetupModeChoice::Individual => {
            config.environment = Environment::Production;
            if custom_backend.is_some() {
                config.api_url = None;
                std::env::remove_var("RIPPLETIDE_API_URL");
                ui::print_info("Ignoring custom backend for this run and using individual cloud mode.");
            }
            Ok(None)
        }
        SetupModeChoice::Enterprise => {
            let Some(backend_url) = (match custom_backend {
                Some(url) => Some(url),
                None => prompt_enterprise_backend_url()?,
            }) else {
                return Ok(Some(LoginResult {
                    success: false,
                    dashboard_url: None,
                }));
            };
            connect_enterprise_backend(config, backend_url).map(Some)
        }
    }
}

fn prompt_enterprise_email(config: &Config) -> io::Result<Option<String>> {
    if let Some(existing_email) = config.email.as_deref() {
        let reuse = ui::styled_prompt_for_mode(
            ui::ConnectionMode::Enterprise,
            &format!("Use {} for the enterprise backend? (y/n) ", existing_email),
        )?;
        if matches!(reuse.trim().to_lowercase().as_str(), "y" | "yes" | "") {
            return Ok(Some(existing_email.to_string()));
        }
    }

    println!("  Enter your email for the enterprise backend");
    let email = ui::styled_prompt_for_mode(ui::ConnectionMode::Enterprise, "")?;
    if email.is_empty() {
        ui::print_error("Email cannot be empty");
        return Ok(None);
    }
    Ok(Some(email))
}

fn connect_enterprise_backend(config: &mut Config, backend_url: String) -> io::Result<LoginResult> {
    let Some(email) = prompt_enterprise_email(config)? else {
        return Ok(LoginResult {
            success: false,
            dashboard_url: None,
        });
    };

    config.environment = Environment::Enterprise;
    config.api_url = Some(backend_url.clone());
    config.user_id = Some(email.clone());
    config.email = Some(email);
    config.session_token = None;
    save_config(config)?;

    println!();
    ui::print_success("Enterprise backend configured");
    ui::print_sub(&format!("Backend: {backend_url}"));
    Ok(LoginResult {
        success: true,
        dashboard_url: None,
    })
}

fn login(config: &mut Config) -> io::Result<LoginResult> {
    let auth_url = config.auth_url().to_string();

    println!("  Enter your email to create your workspace");
    let email = ui::styled_prompt_for_mode(ui::ConnectionMode::Individual, "")?;
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
            let otp =
                ui::styled_prompt_for_mode(ui::ConnectionMode::Individual, "Enter OTP code: ")?;
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
    let resp = match ureq::post(&upload_url())
        .set("Content-Type", &content_type)
        .set("X-User-Id", user_id)
        .send_bytes(&body)
    {
        Ok(resp) => resp,
        Err(ureq::Error::Status(status, resp)) => {
            let raw_body = resp.into_string().unwrap_or_default();
            let body: serde_json::Value = serde_json::from_str(&raw_body).unwrap_or_default();
            let message = body
                .get("message")
                .and_then(|value| value.as_str())
                .or_else(|| body.get("error").and_then(|value| value.as_str()))
                .unwrap_or(if status == 422 {
                    "Not enough stable coding instructions were found to generate rules from this upload."
                } else {
                    "Upload failed."
                });

            let details = body
                .get("messages_extracted")
                .and_then(|value| value.as_u64())
                .map(|count| format!(" Messages extracted: {count}."))
                .unwrap_or_default();

            if status == 422 {
                return Err(format!(
                    "{message} You can still add rules manually below or reconnect after more coding sessions.{details}"
                ));
            }

            return Err(format!("Upload error ({status}): {message}{details}"));
        }
        Err(e) => return Err(format!("Upload error: {e}")),
    };
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

fn normalize_coding_agent_base_url(raw: &str) -> Option<String> {
    let mut value = raw.trim().trim_end_matches('/').to_string();
    if value.is_empty() {
        return None;
    }

    for suffix in [
        UPLOAD_PATH,
        QUERY_RULES_PATH,
        CHECK_CODE_PATH,
        REVIEW_PLAN_BLOCKS_PATH,
        FILES_PATH,
        CLAUDE_MD_CONTRADICTIONS_PATH,
    ] {
        if let Some(prefix) = value.strip_suffix(suffix) {
            let normalized = prefix.trim_end_matches('/').to_string();
            if normalized.is_empty() {
                return None;
            }
            value = normalized;
            break;
        }
    }

    Some(value)
}

fn normalized_endpoint_override(var_name: &str, path: &str) -> Option<String> {
    let raw = std::env::var(var_name).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.trim_end_matches('/').ends_with(path) {
        return Some(trimmed.trim_end_matches('/').to_string());
    }
    let base = normalize_coding_agent_base_url(trimmed)?;
    Some(format!("{base}{path}"))
}

fn custom_coding_agent_backend_override() -> Option<String> {
    let override_base = std::env::var("RIPPLETIDE_API_URL")
        .ok()
        .and_then(|raw| normalize_coding_agent_base_url(&raw))?;
    if override_base == CODING_AGENT_BASE_URL {
        return None;
    }
    Some(override_base)
}

fn active_connection_mode(config: &Config) -> ui::ConnectionMode {
    if custom_coding_agent_backend_override().is_some()
        || matches!(config.environment, Environment::Enterprise)
    {
        ui::ConnectionMode::Enterprise
    } else {
        ui::ConnectionMode::Individual
    }
}

fn print_connection_mode_summary(config: &Config) {
    match active_connection_mode(config) {
        ui::ConnectionMode::Enterprise => {
            let backend = custom_coding_agent_backend_override()
                .or_else(|| config.configured_coding_agent_base_url())
                .unwrap_or_else(|| "custom enterprise backend".to_string());
            let state_line = if config.is_connected() {
                "Connected through company-hosted coding-agent".to_string()
            } else {
                "Custom company backend detected for setup".to_string()
            };
            ui::print_mode_summary(
                ui::ConnectionMode::Enterprise,
                &[
                    state_line,
                    format!("Backend: {backend}"),
                    "Company backend handles the coding-agent flow".to_string(),
                ],
            );
        }
        ui::ConnectionMode::Individual => {
            let state_line = if config.is_connected() {
                "Connected to your Rippletide cloud workspace".to_string()
            } else {
                "Using the standard Rippletide cloud setup".to_string()
            };
            ui::print_mode_summary(
                ui::ConnectionMode::Individual,
                &[
                    state_line,
                    "Email + OTP sign-in through Rippletide".to_string(),
                    "Rules sync with your individual workspace".to_string(),
                ],
            );
        }
    }
}

fn coding_agent_base_url_for_config(config: Option<&Config>) -> String {
    if let Ok(raw) = std::env::var("RIPPLETIDE_API_URL") {
        if let Some(base) = normalize_coding_agent_base_url(&raw) {
            return base;
        }
    }
    if let Ok(raw) = std::env::var("RIPPLETIDE_CODING_AGENT_UPLOAD_URL") {
        if let Some(base) = normalize_coding_agent_base_url(&raw) {
            return base;
        }
    }
    if let Some(base) = config.and_then(|value| value.configured_coding_agent_base_url()) {
        return base;
    }
    CODING_AGENT_BASE_URL.to_string()
}

fn coding_agent_base_url() -> String {
    let config = load_config();
    coding_agent_base_url_for_config(Some(&config))
}

fn claude_md_contradictions_url() -> String {
    normalized_endpoint_override(
        "RIPPLETIDE_CLAUDE_MD_CONTRADICTIONS_URL",
        CLAUDE_MD_CONTRADICTIONS_PATH,
    )
    .unwrap_or_else(|| {
        format!(
            "{}{}",
            coding_agent_base_url(),
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

// --- Post-analysis via claude CLI ---

const QUERY_RULES_PATH: &str = "/query-rules";
const GENERATED_RULES_FILE_ID: &str = "Rules.md";
const GOLD_RULES_FILE_ID: &str = "GoldRules.md";
const LEGACY_SELECTED_RULES_FILE_ID: &str = "SelectedRules.md";
const LOCAL_GOLD_RULES_DIR: &str = ".rippletide";
const LOCAL_GOLD_RULES_PATH: &str = ".rippletide/gold-rules.md";
const LEGACY_LOCAL_SELECTED_RULES_PATH: &str = ".rippletide/selected-rules.md";

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum QuerySource {
    UserPrompt,
    Bootstrap,
}

impl QuerySource {
    fn as_str(self) -> &'static str {
        match self {
            QuerySource::UserPrompt => "user_prompt",
            QuerySource::Bootstrap => "bootstrap",
        }
    }
}

fn upload_url() -> String {
    normalized_endpoint_override("RIPPLETIDE_CODING_AGENT_UPLOAD_URL", UPLOAD_PATH)
        .unwrap_or_else(|| format!("{}{}", coding_agent_base_url(), UPLOAD_PATH))
}

fn query_rules_url() -> String {
    normalized_endpoint_override("RIPPLETIDE_QUERY_RULES_URL", QUERY_RULES_PATH)
        .unwrap_or_else(|| format!("{}{}", coding_agent_base_url(), QUERY_RULES_PATH))
}

// rippletide-override: user approved
fn review_plan_blocks_url() -> String {
    format!("{}{}", coding_agent_base_url(), REVIEW_PLAN_BLOCKS_PATH)
}

fn files_base_url() -> String {
    format!("{}{}", coding_agent_base_url(), FILES_PATH)
}

// rippletide-override: user approved
fn fetch_rules_payload(query: &str, query_source: QuerySource) -> serde_json::Value {
    serde_json::json!({
        "query": query,
        "query_source": query_source.as_str(),
    })
}

fn fetch_rules(user_id: &str, query: &str, query_source: QuerySource) -> FetchRulesResult {
    let url = query_rules_url();
    let payload = fetch_rules_payload(query, query_source);
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

fn fetch_markdown_file(user_id: &str, file_id: &str) -> Result<Option<String>, String> {
    let url = format!("{}/{}", files_base_url(), file_id);
    let resp = match ureq::get(&url).set("X-User-Id", user_id).call() {
        Ok(resp) => resp,
        Err(ureq::Error::Status(404, _)) => return Ok(None),
        Err(ureq::Error::Status(_, resp)) => {
            let body: serde_json::Value = match resp.into_json() {
                Ok(body) => body,
                Err(_) => return Err("api error".to_string()),
            };
            let message = body
                .get("error")
                .and_then(|value| value.as_str())
                .unwrap_or("api error");
            return Err(message.to_string());
        }
        Err(err) => return Err(format!("{err}")),
    };

    let body: serde_json::Value = match resp.into_json() {
        Ok(body) => body,
        Err(err) => return Err(format!("{err}")),
    };

    Ok(body
        .get("file")
        .and_then(|value| value.get("content"))
        .and_then(|value| value.as_str())
        .map(|value| value.to_string()))
}

#[derive(Deserialize)]
struct FilesListEntry {
    id: String,
    content: String,
    #[serde(default)]
    staged: bool,
}

#[derive(Deserialize)]
struct FilesListResponse {
    files: Vec<FilesListEntry>,
}

fn fetch_markdown_files(user_id: &str) -> Result<Vec<FilesListEntry>, String> {
    let resp = match ureq::get(&files_base_url())
        .set("X-User-Id", user_id)
        .call()
    {
        Ok(resp) => resp,
        Err(ureq::Error::Status(_, resp)) => {
            let body: serde_json::Value = match resp.into_json() {
                Ok(body) => body,
                Err(_) => return Err("api error".to_string()),
            };
            let message = body
                .get("error")
                .and_then(|value| value.as_str())
                .unwrap_or("api error");
            return Err(message.to_string());
        }
        Err(err) => return Err(format!("{err}")),
    };

    let body: FilesListResponse = match resp.into_json() {
        Ok(body) => body,
        Err(err) => return Err(format!("{err}")),
    };

    Ok(body.files)
}

fn run_select_files_command(config: &Config, json: bool) -> io::Result<()> {
    let Some(user_id) = config.user_id.as_deref() else {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "not connected; run `rippletide connect` first",
        ));
    };

    let files =
        fetch_markdown_files(user_id).map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
    let selectable_files: Vec<FilesListEntry> = files
        .into_iter()
        .filter(|file| !file.staged && file.id.to_lowercase().ends_with(".md"))
        .collect();

    if selectable_files.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no markdown files are available to share",
        ));
    }

    let items: Vec<String> = selectable_files
        .iter()
        .map(|file| {
            let preview = file
                .content
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .unwrap_or("");
            if preview.is_empty() {
                file.id.clone()
            } else {
                format!("{} - {}", file.id, preview)
            }
        })
        .collect();
    let default_selected: Vec<usize> = selectable_files
        .iter()
        .enumerate()
        .filter_map(|(idx, file)| (file.id == "GoldRules.md").then_some(idx))
        .collect();

    let indices = ui::prompt_multi_select_to_stderr(
        "Pick the files you'd like to share",
        &[
            "Select the markdown files to include. GoldRules.md remains the only active runtime rules file after receive.",
            "Use ↑/↓ to move, space to toggle, enter to confirm, and 'a' to toggle all.",
        ],
        &items,
        "Selected file preview",
        &default_selected,
    )?;

    let selected_ids: Vec<String> = indices
        .into_iter()
        .filter_map(|idx| selectable_files.get(idx))
        .map(|file| file.id.clone())
        .collect();

    if json {
        println!(
            "{}",
            serde_json::to_string(&selected_ids)
                .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?
        );
    } else {
        for file_id in selected_ids {
            println!("{file_id}");
        }
    }

    Ok(())
}

fn parse_list_item(line: &str) -> Option<(usize, String)> {
    let indent = line.chars().take_while(|ch| ch.is_whitespace()).count();
    let trimmed = line[indent..].trim_end();
    let mut chars = trimmed.chars().peekable();
    let marker_len = match chars.peek().copied() {
        Some('-' | '*' | '+') => {
            chars.next();
            if chars
                .peek()
                .copied()
                .map(|ch| ch.is_whitespace())
                .unwrap_or(false)
            {
                1
            } else {
                return None;
            }
        }
        Some(ch) if ch.is_ascii_digit() => {
            let mut digits = 0usize;
            while chars
                .peek()
                .copied()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false)
            {
                chars.next();
                digits += 1;
            }
            match chars.peek().copied() {
                Some('.') | Some(')') => {
                    chars.next();
                    if chars
                        .peek()
                        .copied()
                        .map(|ch| ch.is_whitespace())
                        .unwrap_or(false)
                    {
                        digits + 1
                    } else {
                        return None;
                    }
                }
                _ => return None,
            }
        }
        _ => return None,
    };
    let text = trimmed[marker_len..].trim();
    if text.is_empty() {
        return None;
    }
    Some((indent, text.to_string()))
}

fn parse_rules_markdown(markdown: &str) -> Vec<String> {
    let mut rules = Vec::new();
    let mut in_code_block = false;
    let mut current_parent: Option<String> = None;
    let mut parent_indent: usize = 0;
    let mut children: Vec<String> = Vec::new();

    let flush = |rules: &mut Vec<String>,
                 current_parent: &mut Option<String>,
                 children: &mut Vec<String>| {
        if let Some(parent) = current_parent.take() {
            if children.is_empty() {
                rules.push(parent);
            } else {
                let joined = children.join(", ");
                let base = parent.trim_end().to_string();
                if base.ends_with(':') {
                    rules.push(format!("{base} {joined}"));
                } else if base.ends_with('.') || base.ends_with('!') || base.ends_with('?') {
                    rules.push(format!("{base} {joined}"));
                } else {
                    rules.push(format!("{base}. {joined}"));
                }
            }
        }
        children.clear();
    };

    for line in markdown.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block {
            continue;
        }

        let indent = line.chars().take_while(|ch| ch.is_whitespace()).count();
        if let Some((item_indent, text)) = parse_list_item(line) {
            if current_parent.is_none() {
                if item_indent > 0 {
                    continue;
                }
                current_parent = Some(text);
                parent_indent = item_indent;
                continue;
            }

            if item_indent <= parent_indent {
                flush(&mut rules, &mut current_parent, &mut children);
                if item_indent > 0 {
                    continue;
                }
                current_parent = Some(text);
                parent_indent = item_indent;
            } else {
                children.push(text);
            }
            continue;
        }

        let is_continuation = indent > 0
            && !trimmed.is_empty()
            && parse_list_item(line).is_none()
            && !trimmed.starts_with('#');

        if is_continuation {
            if let Some(last_child) = children.last_mut() {
                last_child.push(' ');
                last_child.push_str(trimmed);
            } else if let Some(parent) = current_parent.as_mut() {
                parent.push(' ');
                parent.push_str(trimmed);
            }
        }
    }

    flush(&mut rules, &mut current_parent, &mut children);
    rules
}

fn fetch_generated_rules(user_id: &str) -> Result<Option<Vec<String>>, String> {
    fetch_markdown_file(user_id, GENERATED_RULES_FILE_ID)
        .map(|content| content.map(|markdown| parse_rules_markdown(&markdown)))
}

fn gold_rules_local_path(cwd: &Path) -> PathBuf {
    cwd.join(LOCAL_GOLD_RULES_PATH)
}

fn gold_rules_markdown(rules: &[String]) -> String {
    let mut content = String::from(
        "# Gold Rules\n\nThese are the final user-approved rules for this repository. Rippletide should use this set as the source of truth.\n\n",
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

fn prompt_rule_selection(
    candidates: &[RuleCandidate],
    current_rules: &[String],
) -> io::Result<Vec<String>> {
    let review = ui::styled_prompt("Would you like to review your Rippletide rules? (y/n)")?;
    if !matches!(review.trim().to_lowercase().as_str(), "y" | "yes") {
        return Ok(current_rules.to_vec());
    }

    let mut final_rules = if candidates.is_empty() {
        if current_rules.is_empty() {
            ui::print_info(
                "No candidate rules were generated automatically. You can add custom rules now.",
            );
        }
        current_rules.to_vec()
    } else {
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

        indices
            .into_iter()
            .filter_map(|idx| candidates.get(idx))
            .map(|candidate| candidate.rule.clone())
            .collect()
    };

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

fn remove_legacy_local_selected_rules(cwd: &Path) {
    let legacy_path = cwd.join(LEGACY_LOCAL_SELECTED_RULES_PATH);
    if legacy_path.exists() {
        let _ = fs::remove_file(legacy_path);
    }
}

fn persist_gold_rules_local(cwd: &Path, rules: &[String]) -> io::Result<PathBuf> {
    let dir = cwd.join(LOCAL_GOLD_RULES_DIR);
    fs::create_dir_all(&dir)?;
    let path = gold_rules_local_path(cwd);
    fs::write(&path, gold_rules_markdown(rules))?;
    remove_legacy_local_selected_rules(cwd);
    Ok(path)
}

fn remove_legacy_selected_rules_remote(user_id: &str) {
    let legacy_url = format!("{}/{}", files_base_url(), LEGACY_SELECTED_RULES_FILE_ID);
    let _ = ureq::delete(&legacy_url).set("X-User-Id", user_id).call();
}

fn persist_gold_rules_remote(user_id: &str, rules: &[String]) -> Result<(), String> {
    let url = format!("{}/{}", files_base_url(), GOLD_RULES_FILE_ID);
    let payload = serde_json::json!({
        "content": gold_rules_markdown(rules),
    });

    match ureq::put(&url)
        .set("Content-Type", "application/json")
        .set("X-User-Id", user_id)
        .send_string(&payload.to_string())
    {
        Ok(_) => {
            remove_legacy_selected_rules_remote(user_id);
            Ok(())
        }
        Err(ureq::Error::Status(404, _)) => {
            let create_url = files_base_url();
            let create_payload = serde_json::json!({
                "id": GOLD_RULES_FILE_ID,
                "content": gold_rules_markdown(rules),
            });
            ureq::post(&create_url)
                .set("Content-Type", "application/json")
                .set("X-User-Id", user_id)
                .send_string(&create_payload.to_string())
                .map_err(|e| format!("Failed to create gold rules remotely: {e}"))?;
            remove_legacy_selected_rules_remote(user_id);
            Ok(())
        }
        Err(e) => Err(format!("Failed to persist gold rules remotely: {e}")),
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

        match fetch_rules(user_id, query, QuerySource::UserPrompt) {
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

struct LivePlanReviewer {
    user_id: Option<String>,
}

impl planner::PlanReviewer for LivePlanReviewer {
    fn review_blocks(&self, blocks: &[String], rules: &[String]) -> planner::PlanReviewResult {
        let Some(user_id) = self.user_id.as_deref() else {
            return planner::PlanReviewResult::Review(planner::PlanReview {
                pass: true,
                violations: Vec::new(),
            });
        };

        let url = review_plan_blocks_url();
        let payload = serde_json::json!({
            "blocks": blocks,
            "rules": rules,
        });
        let resp = match ureq::post(&url)
            .set("Content-Type", "application/json")
            .set("X-User-Id", user_id)
            .send_string(&payload.to_string())
        {
            Ok(r) => r,
            Err(e) => return planner::PlanReviewResult::Error(format!("{e}")),
        };
        let body: serde_json::Value = match resp.into_json() {
            Ok(b) => b,
            Err(e) => return planner::PlanReviewResult::Error(format!("{e}")),
        };

        let pass = body.get("pass").and_then(|v| v.as_bool()).unwrap_or(true);
        let violations = body
            .get("violations")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| {
                        Some(planner::PlanViolation {
                            rule: v.get("rule")?.as_str()?.to_string(),
                            issue: v.get("issue")?.as_str()?.to_string(),
                            fix: v
                                .get("fix")
                                .and_then(|f| f.as_str())
                                .unwrap_or("")
                                .to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        planner::PlanReviewResult::Review(planner::PlanReview { pass, violations })
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
    let reviewer = LivePlanReviewer {
        user_id: config.user_id.clone(),
    };

    let outcome = planner::run_plan_loop(
        cwd,
        &query,
        max_iterations,
        &claude,
        &rules_provider,
        &reviewer,
    )
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

    let claude = LiveClaude; // rippletide-override: user approved
    let rules_provider = LiveRulesProvider {
        user_id: config.user_id.clone(),
    };
    let reviewer = LivePlanReviewer {
        user_id: config.user_id.clone(),
    };

    let review =
        planner::review_plan_candidate(cwd, &query, &plan, &claude, &rules_provider, &reviewer)
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

    if let Some(Commands::SelectFiles { json }) = &cli.command {
        return run_select_files_command(&config, *json);
    }

    let _read_only = match &cli.command {
        Some(Commands::Connect { read_only }) => *read_only,
        None => false,
        Some(Commands::Plan { .. }) => false,
        Some(Commands::ReviewPlan { .. }) => false,
        Some(Commands::SelectFiles { .. }) => false,
        Some(Commands::Logout) => unreachable!(),
    };

    // Phase 1 — Header
    let prompt_setup_mode = should_prompt_setup_mode(&config);
    if prompt_setup_mode {
        ui::print_header("Rippletide Code");
    } else {
        ui::print_mode_header(active_connection_mode(&config));
        print_connection_mode_summary(&config);
    }

    let mut dashboard_url: Option<String> = None;
    let setup_result = select_setup_mode(&mut config)?;
    if let Some(login_result) = setup_result {
        println!();
        if !login_result.success {
            return Ok(());
        }
        dashboard_url = login_result.dashboard_url;
        ui::print_mode_header(active_connection_mode(&config));
        print_connection_mode_summary(&config);
    } else if prompt_setup_mode {
        ui::print_mode_header(active_connection_mode(&config));
        print_connection_mode_summary(&config);
    }

    let is_logged_in = config.is_connected();

    // Phase 2 — Auth (if not logged in)
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
    // Phase 6a — Load the full generated Rules.md document for setup selection
    let sp = ui::start_spinner("Loading generated rules…");
    let mut has_generated_rules = true;
    let mut generated_rules: Vec<String> =
        match config.user_id.as_deref().map(fetch_generated_rules) {
            Some(Ok(Some(rules))) => rules,
            Some(Ok(None)) => {
                has_generated_rules = false;
                Vec::new()
            }
            Some(Err(_)) | None => Vec::new(),
        };
    ui::finish_spinner(&sp, "Generated rules loaded");

    // Phase 6b — Upload sessions to build graph if no generated rules exist, then re-fetch
    if !has_generated_rules || !is_logged_in {
        if let Some(ref uid) = config.user_id {
            println!();
            if !has_generated_rules {
                ui::print_sub("No context graph found — uploading sessions to build one...");
            }
            upload_sessions(uid, &cwd)?;
        }
    }

    if !has_generated_rules {
        let sp = ui::start_spinner("Loading generated rules from new graph…");
        generated_rules = match config.user_id.as_deref().map(fetch_generated_rules) {
            Some(Ok(Some(rules))) => rules,
            _ => generated_rules,
        };
        ui::finish_spinner(&sp, "Generated rules loaded");
    }

    {
        let sp = ui::start_spinner("Building Rippletide Context Graph");
        thread::sleep(Duration::from_millis(800));
        ui::finish_spinner(&sp, "Building Rippletide Context Graph");
        ui::print_success("Context Graph built.");
    }

    println!();

    let candidate_rules = build_rule_candidates(&generated_rules, &[], &[]);
    let current_rules = generated_rules.clone();
    let gold_rules = prompt_rule_selection(&candidate_rules, &current_rules)?;

    if gold_rules.is_empty() {
        ui::print_info("No rules selected yet — skipping gold rules save");
    } else {
        match persist_gold_rules_local(&cwd, &gold_rules) {
            Ok(path) => {
                ui::print_success(&format!("Saved gold rules locally to {}", path.display()))
            }
            Err(err) => ui::print_error(&format!("Failed to save gold rules locally: {err}")),
        }

        if let Some(ref user_id) = config.user_id {
            match persist_gold_rules_remote(user_id, &gold_rules) {
                Ok(()) => ui::print_success("Saved gold rules to Rippletide backend"),
                Err(err) => ui::print_error(&err),
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
    use std::io::{Read, Write};
    use std::net::TcpListener;
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

    fn serve_single_http_response(
        status: &str,
        body: &str,
    ) -> (String, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let response_status = status.to_string();
        let response_body = body.to_string();

        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0_u8; 4096];
            let bytes_read = stream.read(&mut buffer).unwrap();
            let request = String::from_utf8_lossy(&buffer[..bytes_read]).to_string();
            let response = format!(
                "HTTP/1.1 {response_status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response_body}",
                response_body.as_bytes().len()
            );
            stream.write_all(response.as_bytes()).unwrap();
            request
        });

        (format!("http://{address}/upload"), handle)
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
    fn gold_rules_markdown_contains_all_rules() {
        let markdown = gold_rules_markdown(&vec![
            "Keep handlers small".to_string(),
            "Write focused tests".to_string(),
        ]);
        assert!(markdown.contains("# Gold Rules"));
        assert!(markdown.contains("- Keep handlers small"));
        assert!(markdown.contains("- Write focused tests"));
    }

    #[test]
    fn fetch_rules_payload_includes_query_source() {
        // rippletide-override: user approved
        let payload = fetch_rules_payload("Return all coding rules", QuerySource::Bootstrap);
        assert_eq!(payload["query"], "Return all coding rules");
        assert_eq!(payload["query_source"], "bootstrap");
    }

    #[test]
    fn parse_rules_markdown_keeps_all_rule_bullets() {
        let markdown = r#"# Rules

## Priority of Instruction Sources (Hook-First)
### When to apply hook-first behavior
- Before answering any request that may lead to planning, code generation, refactoring, architecture, or tests, first use the hook-injected context tagged `[Coding Rules from Rippletide]` when it is present.
- Treat plan-style requests as including (at minimum):
- /plan ...
- Requests that ask for a step-by-step implementation plan

## Coding and Implementation
### Code on request
- When the user asks to code something, produce the requested code artifact directly.
- Keep the code small and practical.
```md
- ignore code block bullets
```
"#;

        let rules = parse_rules_markdown(markdown);
        assert_eq!(
            rules,
            vec![
                "Before answering any request that may lead to planning, code generation, refactoring, architecture, or tests, first use the hook-injected context tagged `[Coding Rules from Rippletide]` when it is present.".to_string(),
                "Treat plan-style requests as including (at minimum):".to_string(),
                "/plan ...".to_string(),
                "Requests that ask for a step-by-step implementation plan".to_string(),
                "When the user asks to code something, produce the requested code artifact directly.".to_string(),
                "Keep the code small and practical.".to_string(),
            ]
        );
    }

    #[test]
    fn parse_rules_markdown_keeps_continuations_and_merges_nested_bullets() {
        let markdown = r#"# Rules
- Keep handlers small.
  Continue only when the logic stays cohesive.
- Prefer typed boundaries.
  - Nested bullet that should not become a standalone rule.
## Section
- Write focused tests.
"#;
        let parsed = parse_rules_markdown(markdown);
        assert_eq!(parsed.len(), 3);
        assert_eq!(
            parsed[0],
            "Keep handlers small. Continue only when the logic stays cohesive."
        );
        assert_eq!(
            parsed[1],
            "Prefer typed boundaries. Nested bullet that should not become a standalone rule."
        );
        assert_eq!(parsed[2], "Write focused tests.");
    }

    #[test]
    fn parse_rules_markdown_merges_nested_bullets_after_top_level_rule() {
        let markdown = r#"# Rules
- Prefer typed boundaries.
  - Use zod at API boundaries.
  - Reuse validators.
- Keep handlers small.
"#;
        let parsed = parse_rules_markdown(markdown);
        assert_eq!(
            parsed,
            vec![
                "Prefer typed boundaries. Use zod at API boundaries., Reuse validators."
                    .to_string(),
                "Keep handlers small.".to_string(),
            ]
        );
    }

    #[test]
    fn parse_rules_markdown_keeps_mixed_top_level_bullets() {
        let markdown = r#"# Rules
- Keep handlers small.
* Write focused tests.
- Prefer explicit names.
"#;
        let parsed = parse_rules_markdown(markdown);
        assert_eq!(
            parsed,
            vec![
                "Keep handlers small.".to_string(),
                "Write focused tests.".to_string(),
                "Prefer explicit names.".to_string(),
            ]
        );
    }

    #[test]
    fn parse_rules_markdown_ignores_code_blocks_and_headings() {
        let markdown = r#"# Rules
- Keep handlers small.

```md
- This should not be parsed.
```

## Section
- Prefer typed boundaries.
"#;
        let parsed = parse_rules_markdown(markdown);
        assert_eq!(
            parsed,
            vec![
                "Keep handlers small.".to_string(),
                "Prefer typed boundaries.".to_string(),
            ]
        );
    }

    #[test]
    fn parse_rules_markdown_does_not_promote_indented_bullets_without_parent_context() {
        let markdown = r#"  - Nested only bullet
    - Deep nested bullet
- Real top-level rule
"#;
        let parsed = parse_rules_markdown(markdown);
        assert_eq!(parsed, vec!["Real top-level rule".to_string()]);
    }

    #[test]
    fn parse_rules_markdown_keeps_long_continuation_as_same_rule() {
        let markdown = r#"- Prefer explicit names for exported functions.
  Continue the same rule with more explanation so the item stays coherent.
  Another sentence that should remain attached to the same rule.
- Write focused tests.
"#;
        let parsed = parse_rules_markdown(markdown);
        assert_eq!(parsed, vec![
            "Prefer explicit names for exported functions. Continue the same rule with more explanation so the item stays coherent. Another sentence that should remain attached to the same rule.".to_string(),
            "Write focused tests.".to_string(),
        ]);
    }

    #[test]
    fn parse_rules_markdown_keeps_hierarchical_rule_as_one_rule() {
        let markdown = r#"# Rules
- Always do in order:
  - 1
  - 2
  - 3
"#;
        let parsed = parse_rules_markdown(markdown);
        assert_eq!(parsed, vec!["Always do in order: 1, 2, 3".to_string()]);
    }

    #[test]
    fn fetch_generated_rules_reads_rules_file_content() {
        let _guard = env_lock();
        let (base_url, server) = serve_single_http_response(
            "200 OK",
            r##"{"file":{"id":"Rules.md","content":"# Rules\n- Keep handlers small\n- Write focused tests\n"}}"##,
        );
        let old_upload_url = std::env::var_os("RIPPLETIDE_CODING_AGENT_UPLOAD_URL");
        std::env::set_var("RIPPLETIDE_CODING_AGENT_UPLOAD_URL", &base_url);

        let rules = fetch_generated_rules("user-123").unwrap().unwrap();
        assert_eq!(
            rules,
            vec![
                "Keep handlers small".to_string(),
                "Write focused tests".to_string(),
            ]
        );

        let request = server.join().unwrap();
        assert!(request.starts_with("GET /files/Rules.md HTTP/1.1"));
        assert!(request.contains("X-User-Id: user-123"));

        match old_upload_url {
            Some(value) => std::env::set_var("RIPPLETIDE_CODING_AGENT_UPLOAD_URL", value),
            None => std::env::remove_var("RIPPLETIDE_CODING_AGENT_UPLOAD_URL"),
        }
    }

    #[test]
    fn build_rule_candidates_from_full_rules_markdown_keeps_later_sections() {
        let generated_rules = parse_rules_markdown(
            r#"# Rules

## Hook-First
### Required behavior for code-generation responses
- Read the hook-injected rules first.

## Models, Providers, and Configuration
### Cloud limits investigation
- When asked about quotas/limits, investigate and report what is and is not exposed by the provider tooling.
- Summarize findings.
"#,
        );

        let candidates = build_rule_candidates(&generated_rules, &[], &[]);
        assert_eq!(candidates.len(), 3);
        assert_eq!(
            candidates[1].rule,
            "When asked about quotas/limits, investigate and report what is and is not exposed by the provider tooling."
        );
        assert_eq!(candidates[2].rule, "Summarize findings.");
    }

    #[test]
    fn hook_script_tags_prompt_queries_as_user_prompts() {
        assert!(HOOK_SCRIPT.contains("query_source"));
        assert!(HOOK_SCRIPT.contains("\"user_prompt\""));
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
    fn coding_agent_urls_follow_rippletide_api_url_override() {
        let _guard = env_lock();
        let old_api_url = std::env::var_os("RIPPLETIDE_API_URL");
        let old_upload_url = std::env::var_os("RIPPLETIDE_CODING_AGENT_UPLOAD_URL");
        let old_query_rules_url = std::env::var_os("RIPPLETIDE_QUERY_RULES_URL");
        let old_contradictions_url = std::env::var_os("RIPPLETIDE_CLAUDE_MD_CONTRADICTIONS_URL");
        std::env::set_var("RIPPLETIDE_API_URL", "http://127.0.0.1:5051/backend/");
        std::env::remove_var("RIPPLETIDE_CODING_AGENT_UPLOAD_URL");
        std::env::remove_var("RIPPLETIDE_QUERY_RULES_URL");
        std::env::remove_var("RIPPLETIDE_CLAUDE_MD_CONTRADICTIONS_URL");

        assert_eq!(upload_url(), "http://127.0.0.1:5051/backend/upload");
        assert_eq!(
            query_rules_url(),
            "http://127.0.0.1:5051/backend/query-rules"
        );
        assert_eq!(
            review_plan_blocks_url(),
            "http://127.0.0.1:5051/backend/review-plan-blocks"
        );
        assert_eq!(files_base_url(), "http://127.0.0.1:5051/backend/files");
        assert_eq!(
            claude_md_contradictions_url(),
            "http://127.0.0.1:5051/backend/claude-md/contradictions"
        );

        match old_api_url {
            Some(v) => std::env::set_var("RIPPLETIDE_API_URL", v),
            None => std::env::remove_var("RIPPLETIDE_API_URL"),
        }
        match old_upload_url {
            Some(v) => std::env::set_var("RIPPLETIDE_CODING_AGENT_UPLOAD_URL", v),
            None => std::env::remove_var("RIPPLETIDE_CODING_AGENT_UPLOAD_URL"),
        }
        match old_query_rules_url {
            Some(v) => std::env::set_var("RIPPLETIDE_QUERY_RULES_URL", v),
            None => std::env::remove_var("RIPPLETIDE_QUERY_RULES_URL"),
        }
        match old_contradictions_url {
            Some(v) => std::env::set_var("RIPPLETIDE_CLAUDE_MD_CONTRADICTIONS_URL", v),
            None => std::env::remove_var("RIPPLETIDE_CLAUDE_MD_CONTRADICTIONS_URL"),
        }
    }

    #[test]
    fn config_is_connected_for_enterprise_without_session_token() {
        let config = Config {
            user_id: Some("alice@example.com".to_string()),
            session_token: None,
            email: Some("alice@example.com".to_string()),
            api_url: Some("https://coding-agent-staging.up.railway.app".to_string()),
            environment: Environment::Enterprise,
        };

        assert!(config.is_connected());
    }

    #[test]
    fn active_connection_mode_is_enterprise_for_saved_enterprise_config() {
        let config = Config {
            user_id: Some("alice@example.com".to_string()),
            session_token: None,
            email: Some("alice@example.com".to_string()),
            api_url: Some("https://coding-agent-staging.up.railway.app".to_string()),
            environment: Environment::Enterprise,
        };

        assert_eq!(
            active_connection_mode(&config),
            ui::ConnectionMode::Enterprise
        );
    }

    #[test]
    fn active_connection_mode_defaults_to_individual() {
        let config = Config::default();
        assert_eq!(
            active_connection_mode(&config),
            ui::ConnectionMode::Individual
        );
    }

    #[test]
    fn parse_setup_mode_choice_defaults_to_individual() {
        assert_eq!(parse_setup_mode_choice(""), Some(SetupModeChoice::Individual));
        assert_eq!(parse_setup_mode_choice("1"), Some(SetupModeChoice::Individual));
        assert_eq!(
            parse_setup_mode_choice("individual"),
            Some(SetupModeChoice::Individual)
        );
    }

    #[test]
    fn parse_setup_mode_choice_accepts_enterprise_inputs() {
        assert_eq!(parse_setup_mode_choice("2"), Some(SetupModeChoice::Enterprise));
        assert_eq!(
            parse_setup_mode_choice("enterprise"),
            Some(SetupModeChoice::Enterprise)
        );
        assert_eq!(parse_setup_mode_choice("e"), Some(SetupModeChoice::Enterprise));
    }

    #[test]
    fn should_prompt_setup_mode_for_unconnected_config() {
        let config = Config::default();
        assert!(should_prompt_setup_mode(&config));
    }

    #[test]
    fn custom_coding_agent_backend_override_ignores_default_backend() {
        let _guard = env_lock();
        let old_api_url = std::env::var_os("RIPPLETIDE_API_URL");
        std::env::set_var("RIPPLETIDE_API_URL", "https://coding-agent.up.railway.app");

        assert_eq!(custom_coding_agent_backend_override(), None);

        match old_api_url {
            Some(v) => std::env::set_var("RIPPLETIDE_API_URL", v),
            None => std::env::remove_var("RIPPLETIDE_API_URL"),
        }
    }

    #[test]
    fn custom_coding_agent_backend_override_accepts_custom_backend() {
        let _guard = env_lock();
        let old_api_url = std::env::var_os("RIPPLETIDE_API_URL");
        std::env::set_var(
            "RIPPLETIDE_API_URL",
            "https://coding-agent-staging.up.railway.app/backend/",
        );

        assert_eq!(
            custom_coding_agent_backend_override(),
            Some("https://coding-agent-staging.up.railway.app/backend".to_string())
        );

        match old_api_url {
            Some(v) => std::env::set_var("RIPPLETIDE_API_URL", v),
            None => std::env::remove_var("RIPPLETIDE_API_URL"),
        }
    }

    #[test]
    fn coding_agent_base_url_uses_saved_enterprise_backend_when_no_env_override() {
        let _guard = env_lock();
        let old_api_url = std::env::var_os("RIPPLETIDE_API_URL");
        let old_upload_url = std::env::var_os("RIPPLETIDE_CODING_AGENT_UPLOAD_URL");
        std::env::remove_var("RIPPLETIDE_API_URL");
        std::env::remove_var("RIPPLETIDE_CODING_AGENT_UPLOAD_URL");

        let config = Config {
            user_id: Some("alice@example.com".to_string()),
            session_token: None,
            email: Some("alice@example.com".to_string()),
            api_url: Some("https://coding-agent-staging.up.railway.app/backend/".to_string()),
            environment: Environment::Enterprise,
        };

        assert_eq!(
            coding_agent_base_url_for_config(Some(&config)),
            "https://coding-agent-staging.up.railway.app/backend"
        );

        match old_api_url {
            Some(v) => std::env::set_var("RIPPLETIDE_API_URL", v),
            None => std::env::remove_var("RIPPLETIDE_API_URL"),
        }
        match old_upload_url {
            Some(v) => std::env::set_var("RIPPLETIDE_CODING_AGENT_UPLOAD_URL", v),
            None => std::env::remove_var("RIPPLETIDE_CODING_AGENT_UPLOAD_URL"),
        }
    }

    #[test]
    fn upload_zip_surfaces_backend_message_for_insufficient_content() {
        let _guard = env_lock();
        let old_value = std::env::var_os("RIPPLETIDE_CODING_AGENT_UPLOAD_URL");
        let (base_url, handle) = serve_single_http_response(
            "422 Unprocessable Entity",
            r#"{"error":"insufficient_content","message":"Not enough stable coding instructions were found to generate rules from this upload.","messages_extracted":1}"#,
        );
        std::env::set_var("RIPPLETIDE_CODING_AGENT_UPLOAD_URL", &base_url);

        let error = upload_zip(b"fake-zip", "user-123", None).unwrap_err();
        assert!(error.contains("Not enough stable coding instructions were found"));
        assert!(error.contains("add rules manually below"));

        let request = handle.join().unwrap();
        assert!(request.starts_with("POST /upload "));

        match old_value {
            Some(v) => std::env::set_var("RIPPLETIDE_CODING_AGENT_UPLOAD_URL", v),
            None => std::env::remove_var("RIPPLETIDE_CODING_AGENT_UPLOAD_URL"),
        }
    }

    #[test]
    fn upload_url_override_accepts_base_url_value() {
        let _guard = env_lock();
        let old_value = std::env::var_os("RIPPLETIDE_CODING_AGENT_UPLOAD_URL");
        std::env::set_var(
            "RIPPLETIDE_CODING_AGENT_UPLOAD_URL",
            "http://127.0.0.1:5051/backend",
        );

        let value = upload_url();
        assert_eq!(value, "http://127.0.0.1:5051/backend/upload");

        match old_value {
            Some(v) => std::env::set_var("RIPPLETIDE_CODING_AGENT_UPLOAD_URL", v),
            None => std::env::remove_var("RIPPLETIDE_CODING_AGENT_UPLOAD_URL"),
        }
    }

    #[test]
    fn claude_md_contradictions_url_defaults_from_upload_url() {
        let _guard = env_lock();
        let home = temp_dir("rippletide-claude-md-default-url");
        let old_home = std::env::var_os("HOME");
        let old_api_url = std::env::var_os("RIPPLETIDE_API_URL");
        let old_upload_url = std::env::var_os("RIPPLETIDE_CODING_AGENT_UPLOAD_URL");
        let old_contradictions_url = std::env::var_os("RIPPLETIDE_CLAUDE_MD_CONTRADICTIONS_URL");
        std::env::set_var("HOME", &home);
        std::env::remove_var("RIPPLETIDE_API_URL");
        std::env::remove_var("RIPPLETIDE_CODING_AGENT_UPLOAD_URL");
        std::env::remove_var("RIPPLETIDE_CLAUDE_MD_CONTRADICTIONS_URL");

        let value = claude_md_contradictions_url();
        assert_eq!(
            value,
            "https://coding-agent.up.railway.app/claude-md/contradictions"
        );

        match old_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        match old_api_url {
            Some(v) => std::env::set_var("RIPPLETIDE_API_URL", v),
            None => std::env::remove_var("RIPPLETIDE_API_URL"),
        }
        match old_upload_url {
            Some(v) => std::env::set_var("RIPPLETIDE_CODING_AGENT_UPLOAD_URL", v),
            None => std::env::remove_var("RIPPLETIDE_CODING_AGENT_UPLOAD_URL"),
        }
        match old_contradictions_url {
            Some(v) => std::env::set_var("RIPPLETIDE_CLAUDE_MD_CONTRADICTIONS_URL", v),
            None => std::env::remove_var("RIPPLETIDE_CLAUDE_MD_CONTRADICTIONS_URL"),
        }
        let _ = fs::remove_dir_all(home);
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

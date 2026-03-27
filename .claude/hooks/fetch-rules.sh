#!/bin/bash

# Read hook input from stdin
raw_input=$(cat)
if [[ -z "${raw_input//[[:space:]]/}" ]]; then
  exit 0
fi

# Claude Code passes JSON with a "prompt" field to UserPromptSubmit hooks
hook_input=$(echo "$raw_input" | jq -r '.prompt // empty' 2>/dev/null)
if [[ -z "$hook_input" ]]; then
  hook_input="$raw_input"
fi

# Skip commands handled by dedicated hooks
case "$hook_input" in
  invite-rules*|receive-rules*) exit 0 ;;
esac

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
# rippletide-override: user approved
PAYLOAD=$(jq -Rn \
  --arg query "$hook_input" \
  --arg query_source "user_prompt" \
  '{query: $query, query_source: $query_source}' 2>/dev/null)
if [[ -z "$PAYLOAD" ]]; then
  exit 0
fi

RESPONSE=$(curl -s --max-time 180 -X POST "https://coding-agent.up.railway.app/query-rules" \
  -H "Content-Type: application/json" \
  -H "X-User-Id: $USER_ID" \
  -d "$PAYLOAD" 2>/dev/null)

# rippletide-override: user approved
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

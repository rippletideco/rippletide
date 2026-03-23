#!/bin/bash

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

# rippletide-override: user approved
ANSWER=$(echo "$RESPONSE" | jq -r '.answer // empty' 2>/dev/null)
RULE_MGMT=$(echo "$RESPONSE" | jq -r '.rule_management // empty' 2>/dev/null)

if [[ -z "$ANSWER" && -z "$RULE_MGMT" ]]; then
  exit 0
fi

# Return as plain text with explicit instruction
cat <<HOOK_EOF
<user-prompt-submit-hook>
[Coding Rules from Rippletide]

IMPORTANT: You MUST begin your response by listing which of these rules you are applying.
Then ensure ALL generated code complies with these rules.

$ANSWER
HOOK_EOF

# Append rule management instructions if present
if [[ -n "$RULE_MGMT" ]]; then
  cat <<HOOK_EOF

---
$RULE_MGMT

IMPORTANT: When you see [Rule Management] above, you MUST:
1. Present the proposed action and any conflicts to the user clearly.
2. Ask for explicit confirmation before proceeding.
3. For auto-generated files (Rules.md, work_rules_report.md), ask the user to confirm TWICE.
4. Only run the manage-rule.sh command AFTER the user confirms.
HOOK_EOF
fi

echo '</user-prompt-submit-hook>'

#!/bin/bash

# Read hook input from stdin
hook_input=$(cat)

# Read user_id from Rippletide config (macOS or Linux)
CONFIG_FILE="$HOME/Library/Application Support/com.Rippletide.Rippletide/config.json"
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

# Query coding rules
RESPONSE=$(curl -s --max-time 10 -X POST "https://coding-agent-staging.up.railway.app/query" \
  -H "Content-Type: application/json" \
  -H "X-User-Id: $USER_ID" \
  -d '{"query":"What are all the coding rules and conventions?","beam_width":5,"beam_max_depth":8}' 2>/dev/null)

ANSWER=$(echo "$RESPONSE" | jq -r '.answer // empty' 2>/dev/null)
if [[ -z "$ANSWER" ]]; then
  exit 0
fi

# Return as plain text with explicit instruction
cat <<EOF
<user-prompt-submit-hook>
[Coding Rules from Rippletide]

IMPORTANT: You MUST begin your response by listing which of these rules you are applying.
Then ensure ALL generated code complies with these rules.

$ANSWER
</user-prompt-submit-hook>
EOF

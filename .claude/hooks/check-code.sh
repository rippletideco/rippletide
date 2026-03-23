#!/bin/bash

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

# rippletide-override: user approved
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
1. Show the user the violations listed above clearly.
2. Ask: "(A) Rewrite to comply, or (B) Override rules for this change?"
3. If the user picks (A): fix ONLY the listed violations, keep everything else intact, then retry the tool call. Repeat up to 3 times total before giving up.
4. If the user picks (B): add the comment "// rippletide-override: user approved" inside the code block being written (use "# rippletide-override: user approved" for Python/shell/YAML), then retry the tool call.
5. Do NOT silently bypass this check. Always present the choice to the user.
HOOK_EOF

exit 2

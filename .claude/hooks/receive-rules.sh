#!/bin/bash

# UserPromptSubmit hook — handles /receive-rules command.
# Receiver enters OTP, backend stages files, returns conflict report.
# Then receiver chooses to activate or reject.

raw_input=$(cat)
if [[ -z "${raw_input//[[:space:]]/}" ]]; then
  exit 0
fi

# Claude Code passes JSON with a "prompt" field to UserPromptSubmit hooks
prompt=$(echo "$raw_input" | jq -r '.prompt // empty' 2>/dev/null)
if [[ -z "$prompt" ]]; then
  prompt="$raw_input"
fi

# Only trigger on receive-rules command
case "$prompt" in
  receive-rules*) ;;
  *) exit 0 ;;
esac

# Extract OTP from the prompt (e.g., "receive-rules 123456")
OTP_CODE=$(echo "$prompt" | sed 's|^receive-rules[[:space:]]*||' | tr -d '[:space:]')

# Read user config
CONFIG_FILE="$HOME/Library/Application Support/com.Rippletide.Rippletide/config.json"
if [[ ! -f "$CONFIG_FILE" ]]; then
  CONFIG_FILE="$HOME/.config/rippletide/config.json"
fi
if [[ ! -f "$CONFIG_FILE" ]]; then
  CONFIG_FILE="$HOME/.config/Rippletide/Rippletide/config.json"
fi
if [[ ! -f "$CONFIG_FILE" ]]; then
  cat <<'HOOK_EOF'
<user-prompt-submit-hook>
[Rippletide] Not connected. Run `npx rippletide-code@latest connect` first.
</user-prompt-submit-hook>
HOOK_EOF
  exit 0
fi

USER_ID=$(jq -r '.user_id // empty' "$CONFIG_FILE" 2>/dev/null)
if [[ -z "$USER_ID" ]]; then
  cat <<'HOOK_EOF'
<user-prompt-submit-hook>
[Rippletide] Missing credentials. Run `npx rippletide-code@latest connect` first.
</user-prompt-submit-hook>
HOOK_EOF
  exit 0
fi

# If no OTP provided, ask Claude to collect it
if [[ -z "$OTP_CODE" ]]; then
  cat <<'HOOK_EOF'
<user-prompt-submit-hook>
[Rippletide — Receive Shared Rules]

To import shared coding rules, I need the OTP code from the email you received.
Please ask the user: "What is the OTP code from the share invite email?"

Once the user provides the OTP, run this command via the Bash tool:
  bash "$CLAUDE_PROJECT_DIR/.claude/hooks/receive-rules.sh" <<< "receive-rules <otp>"

Replace <otp> with the actual code provided by the user.
</user-prompt-submit-hook>
HOOK_EOF
  exit 0
fi

BASE_URL="${RIPPLETIDE_API_URL:-https://coding-agent.up.railway.app}"

# Step 1: Receive — stage files and get conflict report
PAYLOAD=$(jq -n --arg otp "$OTP_CODE" '{otp: $otp}' 2>/dev/null)

RESPONSE=$(curl -s --max-time 60 -X POST "$BASE_URL/share-rules/receive" \
  -H "Content-Type: application/json" \
  -H "X-User-Id: $USER_ID" \
  -d "$PAYLOAD" 2>/dev/null)

ERROR=$(echo "$RESPONSE" | jq -r '.error // empty' 2>/dev/null)
if [[ -n "$ERROR" ]]; then
  cat <<HOOK_EOF
<user-prompt-submit-hook>
[Rippletide — Receive Failed]

$ERROR
</user-prompt-submit-hook>
HOOK_EOF
  exit 0
fi

SHARE_ID=$(echo "$RESPONSE" | jq -r '.share_id // empty' 2>/dev/null)
SENDER_EMAIL=$(echo "$RESPONSE" | jq -r '.sender_email // empty' 2>/dev/null)
STAGED_FILES=$(echo "$RESPONSE" | jq -r '.staged_files[]? // empty' 2>/dev/null)
STAGED_COUNT=$(echo "$RESPONSE" | jq -r '.staged_files | length' 2>/dev/null)

# Conflict report
NEW_COUNT=$(echo "$RESPONSE" | jq -r '.conflict_report.new_rules_count // 0' 2>/dev/null)
DUP_COUNT=$(echo "$RESPONSE" | jq -r '.conflict_report.duplicate_count // 0' 2>/dev/null)
CONFLICT_COUNT=$(echo "$RESPONSE" | jq -r '.conflict_report.conflict_count // 0' 2>/dev/null)

DUPLICATES=$(echo "$RESPONSE" | jq -r '
  .conflict_report.duplicates[]? |
  "  - \"\(.sender_rule)\" (yours ~ theirs, \(.similarity * 100 | floor)% similar)"
' 2>/dev/null)

CONFLICTS=$(echo "$RESPONSE" | jq -r '
  .conflict_report.conflicts[]? |
  "  - Your rule: \"\(.rule_a.text // "unknown")\"\n    Their rule: \"\(.rule_b.text // "unknown")\"\n    -> \(.consequence // "Conflicting instructions")"
' 2>/dev/null)

cat <<HOOK_EOF
<user-prompt-submit-hook>
[Shared Rules from $SENDER_EMAIL — STAGED]

Received $STAGED_COUNT file(s):
$STAGED_FILES

$NEW_COUNT new rule(s), $DUP_COUNT duplicate(s), $CONFLICT_COUNT conflict(s).
HOOK_EOF

if [[ -n "$DUPLICATES" ]]; then
  cat <<HOOK_EOF

DUPLICATES (will be skipped on activate):
$DUPLICATES
HOOK_EOF
fi

if [[ -n "$CONFLICTS" ]]; then
  cat <<HOOK_EOF

CONFLICTS:
$CONFLICTS
HOOK_EOF
fi

cat <<HOOK_EOF

To activate or reject these rules, run one of the following via the Bash tool:

  # Activate — skip duplicates, keep YOUR version on conflicts
  curl -s -X POST "$BASE_URL/share-rules/activate" \
    -H "Content-Type: application/json" \
    -H "X-User-Id: $USER_ID" \
    -d '{"share_id":"$SHARE_ID","action":"activate_keep_mine"}'

  # Activate — skip duplicates, keep THEIR version on conflicts
  curl -s -X POST "$BASE_URL/share-rules/activate" \
    -H "Content-Type: application/json" \
    -H "X-User-Id: $USER_ID" \
    -d '{"share_id":"$SHARE_ID","action":"activate_keep_theirs"}'

  # Reject — delete all staged shared files
  curl -s -X POST "$BASE_URL/share-rules/activate" \
    -H "Content-Type: application/json" \
    -H "X-User-Id: $USER_ID" \
    -d '{"share_id":"$SHARE_ID","action":"reject"}'

Present the conflict report above to the user and ask them to choose:
  (A) Activate — keep YOUR version on conflicts
  (B) Activate — keep THEIR version on conflicts
  (C) Keep staged — review later
  (D) Reject — delete shared files

Then run the appropriate curl command based on their choice.
</user-prompt-submit-hook>
HOOK_EOF

#!/bin/bash

# rippletide-override: user approved
# UserPromptSubmit hook — handles approve-join command.
# Approves a team join request using OTP.
# Usage: approve-join <team_name> <otp> <requester_email>

raw_input=$(cat)
if [[ -z "${raw_input//[[:space:]]/}" ]]; then
  exit 0
fi

prompt=$(echo "$raw_input" | jq -r '.prompt // empty' 2>/dev/null)
if [[ -z "$prompt" ]]; then
  prompt="$raw_input"
fi

# Only trigger on approve-join command
case "$prompt" in
  approve-join*) ;;
  *) exit 0 ;;
esac

ARGS=$(echo "$prompt" | sed 's|^approve-join[[:space:]]*||')
TEAM_NAME=$(echo "$ARGS" | awk '{print $1}')
OTP_CODE=$(echo "$ARGS" | awk '{print $2}')
REQUESTER_EMAIL=$(echo "$ARGS" | awk '{print $3}')

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

if [[ -z "$TEAM_NAME" || -z "$OTP_CODE" || -z "$REQUESTER_EMAIL" ]]; then
  cat <<'HOOK_EOF'
<user-prompt-submit-hook>
[Rippletide — Approve Join]

Usage: approve-join <team_name> <otp> <requester_email>

Please ask the user for the missing information:
- Team name
- OTP code (from the join request email)
- Requester's email address

Then run via the Bash tool:
  bash "$CLAUDE_PROJECT_DIR/.claude/hooks/approve-join.sh" <<< "approve-join <team_name> <otp> <email>"
</user-prompt-submit-hook>
HOOK_EOF
  exit 0
fi

BASE_URL="${RIPPLETIDE_API_URL:-https://coding-agent.up.railway.app}"

PAYLOAD=$(jq -n --arg otp "$OTP_CODE" --arg email "$REQUESTER_EMAIL" \
  '{otp: $otp, requester_email: $email}' 2>/dev/null)

RESPONSE=$(curl -s --max-time 30 -X POST "$BASE_URL/teams/$TEAM_NAME/approve" \
  -H "Content-Type: application/json" \
  -H "X-User-Id: $USER_ID" \
  -d "$PAYLOAD" 2>/dev/null)

SUCCESS=$(echo "$RESPONSE" | jq -r '.success // false' 2>/dev/null)
MEMBER_EMAIL=$(echo "$RESPONSE" | jq -r '.member_email // empty' 2>/dev/null)
ERROR=$(echo "$RESPONSE" | jq -r '.error // empty' 2>/dev/null)

if [[ "$SUCCESS" == "true" ]]; then
  cat <<HOOK_EOF
<user-prompt-submit-hook>
[Rippletide — Join Approved]

$MEMBER_EMAIL has been added to $TEAM_NAME.
They will receive a welcome email with getting-started commands.
</user-prompt-submit-hook>
HOOK_EOF
else
  cat <<HOOK_EOF
<user-prompt-submit-hook>
[Rippletide — Approve Failed]

$ERROR
</user-prompt-submit-hook>
HOOK_EOF
fi

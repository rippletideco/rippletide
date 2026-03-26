#!/bin/bash

# rippletide-override: user approved
# UserPromptSubmit hook — handles join-team command.
# Requests to join a team. OTP is sent to the owner or a designated approver.
# Usage: join-team <team_name> [approver_email]

raw_input=$(cat)
if [[ -z "${raw_input//[[:space:]]/}" ]]; then
  exit 0
fi

prompt=$(echo "$raw_input" | jq -r '.prompt // empty' 2>/dev/null)
if [[ -z "$prompt" ]]; then
  prompt="$raw_input"
fi

# Only trigger on join-team command
case "$prompt" in
  join-team*) ;;
  *) exit 0 ;;
esac

ARGS=$(echo "$prompt" | sed 's|^join-team[[:space:]]*||')
TEAM_NAME=$(echo "$ARGS" | awk '{print $1}' | tr -d '[:space:]')
APPROVER_EMAIL=$(echo "$ARGS" | awk '{print $2}' | tr -d '[:space:]')

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
USER_EMAIL=$(jq -r '.email // empty' "$CONFIG_FILE" 2>/dev/null)
if [[ -z "$USER_ID" || -z "$USER_EMAIL" ]]; then
  cat <<'HOOK_EOF'
<user-prompt-submit-hook>
[Rippletide] Missing credentials. Run `npx rippletide-code@latest connect` first.
</user-prompt-submit-hook>
HOOK_EOF
  exit 0
fi

if [[ -z "$TEAM_NAME" ]]; then
  cat <<'HOOK_EOF'
<user-prompt-submit-hook>
[Rippletide — Join Team]

To join a team, I need the team name.
Please ask the user: "What is the team name you want to join?"

Once the user provides a name, run this command via the Bash tool:
  bash "$CLAUDE_PROJECT_DIR/.claude/hooks/join-team.sh" <<< "join-team <team_name> [approver_email]"

The approver email is optional — if omitted, the request goes to the team owner.
</user-prompt-submit-hook>
HOOK_EOF
  exit 0
fi

BASE_URL="${RIPPLETIDE_API_URL:-https://coding-agent.up.railway.app}"

if [[ -n "$APPROVER_EMAIL" && "$APPROVER_EMAIL" == *"@"* ]]; then
  PAYLOAD=$(jq -n --arg email "$APPROVER_EMAIL" '{approver_email: $email}' 2>/dev/null)
else
  PAYLOAD='{}'
fi

RESPONSE=$(curl -s --max-time 30 -X POST "$BASE_URL/teams/$TEAM_NAME/join" \
  -H "Content-Type: application/json" \
  -H "X-User-Id: $USER_ID" \
  -H "X-User-Email: $USER_EMAIL" \
  -d "$PAYLOAD" 2>/dev/null)

SUCCESS=$(echo "$RESPONSE" | jq -r '.success // false' 2>/dev/null)
MESSAGE=$(echo "$RESPONSE" | jq -r '.message // empty' 2>/dev/null)
ERROR=$(echo "$RESPONSE" | jq -r '.error // empty' 2>/dev/null)

if [[ "$SUCCESS" == "true" ]]; then
  cat <<HOOK_EOF
<user-prompt-submit-hook>
[Rippletide — Join Request Sent]

$MESSAGE
Ask them to run: approve-join $TEAM_NAME <otp> $USER_EMAIL
</user-prompt-submit-hook>
HOOK_EOF
else
  cat <<HOOK_EOF
<user-prompt-submit-hook>
[Rippletide — Join Request Failed]

$ERROR
</user-prompt-submit-hook>
HOOK_EOF
fi

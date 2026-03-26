#!/bin/bash

# rippletide-override: user approved
# UserPromptSubmit hook — handles create-team command.
# Creates a new team with the caller as owner.

raw_input=$(cat)
if [[ -z "${raw_input//[[:space:]]/}" ]]; then
  exit 0
fi

prompt=$(echo "$raw_input" | jq -r '.prompt // empty' 2>/dev/null)
if [[ -z "$prompt" ]]; then
  prompt="$raw_input"
fi

# Only trigger on create-team command
case "$prompt" in
  create-team*) ;;
  *) exit 0 ;;
esac

TEAM_NAME=$(echo "$prompt" | sed 's|^create-team[[:space:]]*||' | awk '{print $1}' | tr -d '[:space:]')

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

# rippletide-override: user approved
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
[Rippletide — Create Team]

To create a team, I need a team name.
Please ask the user: "What would you like to name your team?"

Once the user provides a name, run this command via the Bash tool:
  bash "$CLAUDE_PROJECT_DIR/.claude/hooks/create-team.sh" <<< "create-team <team_name>"

Replace <team_name> with the actual name (alphanumeric, hyphens, underscores).
</user-prompt-submit-hook>
HOOK_EOF
  exit 0
fi

BASE_URL="${RIPPLETIDE_API_URL:-https://coding-agent.up.railway.app}"

PAYLOAD=$(jq -n --arg name "$TEAM_NAME" '{name: $name}' 2>/dev/null)

# rippletide-override: user approved
RESPONSE=$(curl -s --max-time 30 -X POST "$BASE_URL/teams" \
  -H "Content-Type: application/json" \
  -H "X-User-Id: $USER_ID" \
  -H "X-User-Email: $USER_EMAIL" \
  -d "$PAYLOAD" 2>/dev/null)

TEAM_ID=$(echo "$RESPONSE" | jq -r '.team_id // empty' 2>/dev/null)
ERROR=$(echo "$RESPONSE" | jq -r '.error // empty' 2>/dev/null)

if [[ -n "$TEAM_ID" ]]; then
  cat <<HOOK_EOF
<user-prompt-submit-hook>
[Rippletide — Team Created]

Team "$TEAM_NAME" created successfully. You are the owner.

Useful commands:
  push-rules $TEAM_NAME — push your rules to the team
  Others can join with: join-team $TEAM_NAME
</user-prompt-submit-hook>
HOOK_EOF
else
  cat <<HOOK_EOF
<user-prompt-submit-hook>
[Rippletide — Create Team Failed]

Failed to create team: $ERROR

Full response: $RESPONSE
</user-prompt-submit-hook>
HOOK_EOF
fi

#!/bin/bash

# rippletide-override: user approved
# UserPromptSubmit hook — handles push-rules command.
# Pushes user's rule files to a team.
# Usage: push-rules <team_name>

raw_input=$(cat)
if [[ -z "${raw_input//[[:space:]]/}" ]]; then
  exit 0
fi

prompt=$(echo "$raw_input" | jq -r '.prompt // empty' 2>/dev/null)
if [[ -z "$prompt" ]]; then
  prompt="$raw_input"
fi

# Only trigger on push-rules command
case "$prompt" in
  push-rules*) ;;
  *) exit 0 ;;
esac

TEAM_NAME=$(echo "$prompt" | sed 's|^push-rules[[:space:]]*||' | awk '{print $1}' | tr -d '[:space:]')

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

if [[ -z "$TEAM_NAME" ]]; then
  cat <<'HOOK_EOF'
<user-prompt-submit-hook>
[Rippletide — Push Rules]

To push rules to a team, I need the team name.
Please ask the user: "Which team should I push your rules to?"

Once the user provides a name, run this command via the Bash tool:
  bash "$CLAUDE_PROJECT_DIR/.claude/hooks/push-rules.sh" <<< "push-rules <team_name>"
</user-prompt-submit-hook>
HOOK_EOF
  exit 0
fi

BASE_URL="${RIPPLETIDE_API_URL:-https://coding-agent.up.railway.app}"

RESPONSE=$(curl -s --max-time 30 -X POST "$BASE_URL/teams/$TEAM_NAME/push-rules" \
  -H "Content-Type: application/json" \
  -H "X-User-Id: $USER_ID" \
  -d '{}' 2>/dev/null)

PUSHED_FILES=$(echo "$RESPONSE" | jq -r '.pushed_files[]? // empty' 2>/dev/null)
PUSHED_COUNT=$(echo "$RESPONSE" | jq -r '.pushed_files | length' 2>/dev/null)
VERSIONS=$(echo "$RESPONSE" | jq -r '.versions // {}' 2>/dev/null)
ERROR=$(echo "$RESPONSE" | jq -r '.error // empty' 2>/dev/null)

if [[ -n "$PUSHED_FILES" ]]; then
  cat <<HOOK_EOF
<user-prompt-submit-hook>
[Rippletide — Rules Pushed]

Pushed $PUSHED_COUNT file(s) to $TEAM_NAME:
$PUSHED_FILES

Versions: $VERSIONS

Team members can sync with: sync-rules $TEAM_NAME
</user-prompt-submit-hook>
HOOK_EOF
else
  cat <<HOOK_EOF
<user-prompt-submit-hook>
[Rippletide — Push Failed]

$ERROR
</user-prompt-submit-hook>
HOOK_EOF
fi

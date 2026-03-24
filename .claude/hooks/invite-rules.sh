#!/bin/bash

# UserPromptSubmit hook — handles /invite-rules command.
# Collects receiver email from hook input, sends share invite via backend.

raw_input=$(cat)
if [[ -z "${raw_input//[[:space:]]/}" ]]; then
  exit 0
fi

# Claude Code passes JSON with a "prompt" field to UserPromptSubmit hooks
prompt=$(echo "$raw_input" | jq -r '.prompt // empty' 2>/dev/null)
if [[ -z "$prompt" ]]; then
  prompt="$raw_input"
fi

# Only trigger on invite-rules command
case "$prompt" in
  invite-rules*) ;;
  *) exit 0 ;;
esac

# Extract receiver email from the prompt (e.g., "invite-rules bob@co.com")
RECEIVER_EMAIL=$(echo "$prompt" | sed 's|^invite-rules[[:space:]]*||' | tr -d '[:space:]')

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

# If no email provided in command, ask Claude to collect it
if [[ -z "$RECEIVER_EMAIL" || "$RECEIVER_EMAIL" != *"@"* ]]; then
  cat <<'HOOK_EOF'
<user-prompt-submit-hook>
[Rippletide — Share Rules]

To share your coding rules, I need the recipient's email address.
Please ask the user: "What email address should I send the rules to?"

Once the user provides an email, run this command via the Bash tool:
  bash "$CLAUDE_PROJECT_DIR/.claude/hooks/invite-rules.sh" <<< "invite-rules <email>"

Replace <email> with the actual email address provided by the user.
</user-prompt-submit-hook>
HOOK_EOF
  exit 0
fi

BASE_URL="${RIPPLETIDE_API_URL:-https://coding-agent.up.railway.app}"

PAYLOAD=$(jq -n --arg email "$RECEIVER_EMAIL" '{receiver_email: $email}' 2>/dev/null)

RESPONSE=$(curl -s --max-time 30 -X POST "$BASE_URL/share-rules/invite" \
  -H "Content-Type: application/json" \
  -H "X-User-Id: $USER_ID" \
  -H "X-User-Email: $USER_EMAIL" \
  -d "$PAYLOAD" 2>/dev/null)

SUCCESS=$(echo "$RESPONSE" | jq -r '.success // false' 2>/dev/null)
ERROR=$(echo "$RESPONSE" | jq -r '.error // empty' 2>/dev/null)
FILES_COUNT=$(echo "$RESPONSE" | jq -r '.files_count // 0' 2>/dev/null)

if [[ "$SUCCESS" == "true" ]]; then
  cat <<HOOK_EOF
<user-prompt-submit-hook>
[Rippletide — Rules Shared]

Invite sent to $RECEIVER_EMAIL ($FILES_COUNT rule file(s)).
The recipient has 24 hours to enter the OTP code from their email.

Tell the recipient to type "receive-rules" in their Claude session and enter the code.
</user-prompt-submit-hook>
HOOK_EOF
else
  cat <<HOOK_EOF
<user-prompt-submit-hook>
[Rippletide — Share Failed]

Failed to share rules: $ERROR

Full response: $RESPONSE
</user-prompt-submit-hook>
HOOK_EOF
fi

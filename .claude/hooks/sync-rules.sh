#!/bin/bash

# rippletide-override: user approved
# UserPromptSubmit hook — handles sync-rules command.
# Syncs team rules to user's workspace. No OTP needed (membership is sufficient).
# Usage: sync-rules <team_name>

raw_input=$(cat)
if [[ -z "${raw_input//[[:space:]]/}" ]]; then
  exit 0
fi

prompt=$(echo "$raw_input" | jq -r '.prompt // empty' 2>/dev/null)
if [[ -z "$prompt" ]]; then
  prompt="$raw_input"
fi

# Only trigger on sync-rules command
case "$prompt" in
  sync-rules*) ;;
  *) exit 0 ;;
esac

TEAM_NAME=$(echo "$prompt" | sed 's|^sync-rules[[:space:]]*||' | awk '{print $1}' | tr -d '[:space:]')

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
[Rippletide — Sync Rules]

To sync team rules, I need the team name.
Please ask the user: "Which team's rules would you like to sync?"

Once the user provides a name, run this command via the Bash tool:
  bash "$CLAUDE_PROJECT_DIR/.claude/hooks/sync-rules.sh" <<< "sync-rules <team_name>"
</user-prompt-submit-hook>
HOOK_EOF
  exit 0
fi

BASE_URL="${RIPPLETIDE_API_URL:-https://coding-agent.up.railway.app}"

RESPONSE=$(curl -s --max-time 60 -X POST "$BASE_URL/teams/$TEAM_NAME/sync-rules" \
  -H "Content-Type: application/json" \
  -H "X-User-Id: $USER_ID" \
  -d '{}' 2>/dev/null)

ERROR=$(echo "$RESPONSE" | jq -r '.error // empty' 2>/dev/null)
if [[ -n "$ERROR" ]]; then
  # Check for "Already up to date"
  MESSAGE=$(echo "$RESPONSE" | jq -r '.message // empty' 2>/dev/null)
  if [[ -n "$MESSAGE" ]]; then
    cat <<HOOK_EOF
<user-prompt-submit-hook>
[Rippletide — Sync Rules]

$MESSAGE — no changes to apply.
</user-prompt-submit-hook>
HOOK_EOF
  else
    cat <<HOOK_EOF
<user-prompt-submit-hook>
[Rippletide — Sync Failed]

$ERROR
</user-prompt-submit-hook>
HOOK_EOF
  fi
  exit 0
fi

TEAM_ID=$(echo "$RESPONSE" | jq -r '.team_id // empty' 2>/dev/null)
NEW_COUNT=$(echo "$RESPONSE" | jq -r '.new | length' 2>/dev/null)
UPDATED_COUNT=$(echo "$RESPONSE" | jq -r '.updated | length' 2>/dev/null)
DELETED_COUNT=$(echo "$RESPONSE" | jq -r '.deleted | length' 2>/dev/null)
NEW_FILES=$(echo "$RESPONSE" | jq -r '.new[]? // empty' 2>/dev/null)
UPDATED_FILES=$(echo "$RESPONSE" | jq -r '.updated[]? // empty' 2>/dev/null)
DELETED_FILES=$(echo "$RESPONSE" | jq -r '.deleted[]? // empty' 2>/dev/null)

# Conflict report
CR_NEW=$(echo "$RESPONSE" | jq -r '.conflict_report.new_rules_count // 0' 2>/dev/null)
DUP_COUNT=$(echo "$RESPONSE" | jq -r '.conflict_report.duplicate_count // 0' 2>/dev/null)
CONFLICT_COUNT=$(echo "$RESPONSE" | jq -r '.conflict_report.conflict_count // 0' 2>/dev/null)

DUPLICATES=$(echo "$RESPONSE" | jq -r '
  .conflict_report.duplicates[]? |
  "  - \"\(.sender_rule)\" (~\(.similarity * 100 | floor)% similar)"
' 2>/dev/null)

CONFLICTS=$(echo "$RESPONSE" | jq -r '
  .conflict_report.conflicts[]? |
  "  - Your rule: \"\(.rule_a.text // "unknown")\"\n    Team rule: \"\(.rule_b.text // "unknown")\"\n    -> \(.consequence // "Conflicting instructions")"
' 2>/dev/null)

cat <<HOOK_EOF
<user-prompt-submit-hook>
[Rippletide — Team Rules Synced from $TEAM_NAME]

$NEW_COUNT new, $UPDATED_COUNT updated, $DELETED_COUNT deleted.
HOOK_EOF

if [[ -n "$NEW_FILES" ]]; then
  cat <<HOOK_EOF

New files:
$NEW_FILES
HOOK_EOF
fi

if [[ -n "$UPDATED_FILES" ]]; then
  cat <<HOOK_EOF

Updated files:
$UPDATED_FILES
HOOK_EOF
fi

if [[ -n "$DELETED_FILES" ]]; then
  cat <<HOOK_EOF

Deleted files (will be removed on activate):
$DELETED_FILES
HOOK_EOF
fi

if [[ -n "$DUPLICATES" ]]; then
  cat <<HOOK_EOF

DUPLICATES:
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

# rippletide-override: user approved
To activate or decline, run one of the following via the Bash tool:

  # Accept non-conflicting — add new rules, skip conflicts (keep YOUR version)
  curl -s -X POST "$BASE_URL/teams/$TEAM_NAME/sync-rules/activate" \\
    -H "Content-Type: application/json" \\
    -H "X-User-Id: $USER_ID" \\
    -d '{"action":"accept_non_conflicting"}'

  # Accept all — full merge, TEAM version wins on conflicts
  curl -s -X POST "$BASE_URL/teams/$TEAM_NAME/sync-rules/activate" \\
    -H "Content-Type: application/json" \\
    -H "X-User-Id: $USER_ID" \\
    -d '{"action":"accept_all"}'

  # Decline — reject all (won't re-appear on next sync)
  curl -s -X POST "$BASE_URL/teams/$TEAM_NAME/sync-rules/activate" \\
    -H "Content-Type: application/json" \\
    -H "X-User-Id: $USER_ID" \\
    -d '{"action":"decline"}'

Present the sync report above to the user and ask them to choose:
  (A) Accept non-conflicting — add new rules, skip conflicts
  (B) Accept all — team rules win on conflicts
  (C) Keep staged — review later
  (D) Decline — reject all (won't re-appear on next sync)

Then run the appropriate curl command based on their choice.
</user-prompt-submit-hook>
HOOK_EOF

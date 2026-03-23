#!/bin/bash

# Helper script for managing Rippletide coding rules from a Claude Code session.
# Called by Claude via Bash tool after user confirmation.
#
# Usage:
#   manage-rule.sh add "rule text"
#   manage-rule.sh edit "new content" "file-id.md"
#   manage-rule.sh delete "" "file-id.md"

ACTION="$1"
RULE_TEXT="$2"
FILE_ID="${3:-}"

if [[ -z "$ACTION" ]]; then
  echo '{"error":"Missing action. Usage: manage-rule.sh <add|edit|delete> <rule_text> [file_id]"}'
  exit 1
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
  echo '{"error":"Rippletide config not found. Run rippletide-code connect first."}'
  exit 1
fi

USER_ID=$(jq -r '.user_id // empty' "$CONFIG_FILE" 2>/dev/null)
if [[ -z "$USER_ID" ]]; then
  echo '{"error":"No user_id in config. Run rippletide-code connect first."}'
  exit 1
fi

BASE_URL="${RIPPLETIDE_API_URL:-https://coding-agent.up.railway.app}"

case "$ACTION" in
  add)
    if [[ -z "$RULE_TEXT" ]]; then
      echo '{"error":"Missing rule text for add action."}'
      exit 1
    fi
    FILE_ID="session-rule-$(date +%Y%m%d-%H%M%S).md"
    CONTENT=$(printf "# Session Rule\n\n> Added from Claude Code session on %s\n\n- %s" "$(date +%Y-%m-%d)" "$RULE_TEXT")
    PAYLOAD=$(jq -n \
      --arg id "$FILE_ID" \
      --arg content "$CONTENT" \
      '{id: $id, content: $content}')
    RESPONSE=$(curl -s --max-time 30 -X POST "$BASE_URL/files" \
      -H "Content-Type: application/json" \
      -H "X-User-Id: $USER_ID" \
      -d "$PAYLOAD" 2>/dev/null)
    echo "$RESPONSE"
    ;;
  edit)
    if [[ -z "$RULE_TEXT" ]]; then
      echo '{"error":"Missing new content for edit action."}'
      exit 1
    fi
    if [[ -z "$FILE_ID" ]]; then
      echo '{"error":"Missing file_id for edit action."}'
      exit 1
    fi
    ENCODED_ID=$(printf '%s' "$FILE_ID" | jq -sRr @uri)
    PAYLOAD=$(jq -n --arg content "$RULE_TEXT" '{content: $content}')
    RESPONSE=$(curl -s --max-time 30 -X PUT "$BASE_URL/files/$ENCODED_ID" \
      -H "Content-Type: application/json" \
      -H "X-User-Id: $USER_ID" \
      -d "$PAYLOAD" 2>/dev/null)
    echo "$RESPONSE"
    ;;
  delete)
    if [[ -z "$FILE_ID" ]]; then
      echo '{"error":"Missing file_id for delete action."}'
      exit 1
    fi
    ENCODED_ID=$(printf '%s' "$FILE_ID" | jq -sRr @uri)
    RESPONSE=$(curl -s --max-time 30 -X DELETE "$BASE_URL/files/$ENCODED_ID" \
      -H "X-User-Id: $USER_ID" 2>/dev/null)
    echo "$RESPONSE"
    ;;
  *)
    echo '{"error":"Unknown action. Use: add, edit, delete"}'
    exit 1
    ;;
esac

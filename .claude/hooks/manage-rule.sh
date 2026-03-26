#!/bin/bash

# Helper script for managing Rippletide coding rules from a Claude Code session.
# Called by Claude via Bash tool after user confirmation.
#
# rippletide-override: user approved
# Usage:
#   manage-rule.sh add "rule text"
#   manage-rule.sh edit "new content" "file-id.md"
#   manage-rule.sh delete "" "file-id.md"
#   manage-rule.sh edit-rule "old rule text" "new rule text" "file-id.md"
#   manage-rule.sh delete-rule "rule text" "file-id.md"

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
    FILE_ID="GoldRules.md"
    ENCODED_ID=$(printf '%s' "$FILE_ID" | jq -sRr @uri)
    EXISTING_RESPONSE=$(curl -s --max-time 30 -X GET "$BASE_URL/files/$ENCODED_ID" \
      -H "X-User-Id: $USER_ID" 2>/dev/null)
    EXISTING_CONTENT=$(echo "$EXISTING_RESPONSE" | jq -r '.file.content // empty' 2>/dev/null)

    if [[ -n "$EXISTING_CONTENT" ]]; then
      if [[ "$EXISTING_CONTENT" != *$'\n' ]]; then
        EXISTING_CONTENT="${EXISTING_CONTENT}"$'\n'
      fi
      CONTENT="${EXISTING_CONTENT}- ${RULE_TEXT}"$'\n'
      PAYLOAD=$(jq -n --arg content "$CONTENT" '{content: $content}')
      RESPONSE=$(curl -s --max-time 30 -X PUT "$BASE_URL/files/$ENCODED_ID" \
        -H "Content-Type: application/json" \
        -H "X-User-Id: $USER_ID" \
        -d "$PAYLOAD" 2>/dev/null)
    else
      CONTENT=$(printf "# Gold Rules\n\nThese are the final user-approved rules for this repository. Rippletide should use this set as the source of truth.\n\n- %s\n" "$RULE_TEXT")
      PAYLOAD=$(jq -n \
        --arg id "$FILE_ID" \
        --arg content "$CONTENT" \
        '{id: $id, content: $content}')
      RESPONSE=$(curl -s --max-time 30 -X POST "$BASE_URL/files" \
        -H "Content-Type: application/json" \
        -H "X-User-Id: $USER_ID" \
        -d "$PAYLOAD" 2>/dev/null)
    fi
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
  # rippletide-override: user approved
  edit-rule)
    # Edit a single rule within a file (not the whole file)
    OLD_RULE="$2"
    NEW_RULE="$3"
    FILE_ID="$4"
    if [[ -z "$OLD_RULE" || -z "$NEW_RULE" ]]; then
      echo '{"error":"Missing old or new rule text for edit-rule action."}'
      exit 1
    fi
    if [[ -z "$FILE_ID" ]]; then
      echo '{"error":"Missing file_id for edit-rule action."}'
      exit 1
    fi
    ENCODED_ID=$(printf '%s' "$FILE_ID" | jq -sRr @uri)
    PAYLOAD=$(jq -n \
      --arg action "edit_rule" \
      --arg old_rule "$OLD_RULE" \
      --arg new_rule "$NEW_RULE" \
      '{action: $action, old_rule: $old_rule, new_rule: $new_rule}')
    RESPONSE=$(curl -s --max-time 30 -X PATCH "$BASE_URL/files/$ENCODED_ID" \
      -H "Content-Type: application/json" \
      -H "X-User-Id: $USER_ID" \
      -d "$PAYLOAD" 2>/dev/null)
    echo "$RESPONSE"
    ;;
  delete-rule)
    # Delete a single rule from a file (not the whole file)
    RULE_TO_DELETE="$2"
    FILE_ID="$3"
    if [[ -z "$RULE_TO_DELETE" ]]; then
      echo '{"error":"Missing rule text for delete-rule action."}'
      exit 1
    fi
    if [[ -z "$FILE_ID" ]]; then
      echo '{"error":"Missing file_id for delete-rule action."}'
      exit 1
    fi
    ENCODED_ID=$(printf '%s' "$FILE_ID" | jq -sRr @uri)
    PAYLOAD=$(jq -n \
      --arg action "remove_rule" \
      --arg rule_text "$RULE_TO_DELETE" \
      '{action: $action, rule_text: $rule_text}')
    RESPONSE=$(curl -s --max-time 30 -X PATCH "$BASE_URL/files/$ENCODED_ID" \
      -H "Content-Type: application/json" \
      -H "X-User-Id: $USER_ID" \
      -d "$PAYLOAD" 2>/dev/null)
    echo "$RESPONSE"
    ;;
  *)
    echo '{"error":"Unknown action. Use: add, edit, delete, edit-rule, delete-rule"}'
    exit 1
    ;;
esac

#!/bin/bash

# Stop hook: show all changes (tracked + untracked) made during the session
# Writes to stderr so it appears in the terminal, stdout for Claude context

OUTPUT=""

# Tracked file changes (staged + unstaged)
TRACKED_DIFF=$(git diff HEAD 2>/dev/null)
if [[ -n "$TRACKED_DIFF" ]]; then
  TRACKED_STAT=$(git diff HEAD --stat 2>/dev/null)
  OUTPUT+="--- Tracked Changes ---
$TRACKED_STAT

$TRACKED_DIFF"
fi

# New untracked files
UNTRACKED=$(git ls-files --others --exclude-standard 2>/dev/null)
if [[ -n "$UNTRACKED" ]]; then
  if [[ -n "$OUTPUT" ]]; then
    OUTPUT+=$'\n\n'
  fi
  OUTPUT+="--- New Files ---"
  while IFS= read -r file; do
    CONTENT=$(cat "$file" 2>/dev/null)
    OUTPUT+=$'\n'"+ $file"$'\n'"$CONTENT"$'\n'
  done <<< "$UNTRACKED"
fi

if [[ -z "$OUTPUT" ]]; then
  exit 0
fi

# Print to stderr (visible in terminal)
echo "" >&2
echo "╭─── Code Changes ───╮" >&2
echo "$OUTPUT" >&2
echo "╰────────────────────╯" >&2

# Also return to stdout for Claude context
cat <<EOF
[Code Changes]
$OUTPUT
EOF

---
description: Generate a repo-aware implementation plan revised against Rippletide rules
argument-hint: "<request>"
allowed-tools:
  - Bash
---
Return exactly the final revised plan below and nothing else.

Request:
$ARGUMENTS

Final revised plan:
!`bash "${CLAUDE_PROJECT_DIR:-$PWD}/.claude/commands/plan-command.sh" "$ARGUMENTS"`

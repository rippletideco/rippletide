---
description: Generate a repo-aware implementation plan revised against Rippletide rules
argument-hint: "<request>"
disable-model-invocation: true
allowed-tools:
  - Bash
---
Return exactly the final revised plan below and nothing else.

Request:
$ARGUMENTS

Final revised plan:
!`bash "$CLAUDE_PROJECT_DIR/.claude/commands/plan-command.sh" "$ARGUMENTS"`

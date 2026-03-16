---
description: Generate a repo-aware implementation plan revised against Rippletide rules
argument-hint: "<request>"
allowed-tools:
  - Bash(bash "$CLAUDE_PROJECT_DIR/.claude/commands/plan-command.sh":*)
---
Return exactly the final revised plan below and nothing else.

Request:
$ARGUMENTS

Final revised plan:
```text
!`bash "$CLAUDE_PROJECT_DIR/.claude/commands/plan-command.sh" <<'__RIPPLETIDE_PLAN_REQUEST__'
$ARGUMENTS
__RIPPLETIDE_PLAN_REQUEST__`
```

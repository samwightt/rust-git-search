#!/bin/bash
# Claude Code Stop Hook - Code Review Trigger

# Always request code review before stopping
cat >&2 <<'EOF'
Before stopping, you MUST use the code-review skill to review the changes you made this session.

Use the code-review skill now.
EOF
exit 2

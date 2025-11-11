#!/bin/bash
# Claude Code Stop Hook - Code Review Trigger
# This script checks if code changes were made and triggers a review if needed

# Read hook input from stdin (contains session info)
hook_input=$(cat)

# Check if there are any modified or new files in git
if git diff --quiet HEAD && git diff --cached --quiet && [ -z "$(git ls-files --others --exclude-standard)" ]; then
    # No changes detected, allow stop
    exit 0
else
    # Changes detected, block and request code review using the skill
    # Exit code 2 feeds stderr back to Claude for processing
    cat >&2 <<'EOF'
If you are finished coding, you MUST use the code-review skill to perform a review of the changes you made during this session.

Use the code-review skill now.
EOF
    exit 2
fi

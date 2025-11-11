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
    # Changes detected, block and request code review
    # Exit code 2 feeds stderr back to Claude for processing
    cat >&2 <<'EOF'
Please perform a code review of the changes you made during this session.

**Code Review Instructions:**

For each issue or suggestion you find:
1. Describe the issue clearly
2. Explain why it's a concern (performance, maintainability, correctness, style, etc.)
3. Suggest how to fix it

**Important guidelines:**
- Do NOT fix any issues automatically
- Do NOT re-read files unnecessarily to save tokens
- Use your memory of the changes you made during this session
- Number each finding (1, 2, 3, etc.) so the user can reference them
- Keep findings concise and actionable
- Focus on meaningful issues, not nitpicks
- Consider: correctness, performance, security, maintainability, and code style

**After your review:**
- Ask the user which changes they'd like you to make
- They can respond with numbers (e.g., "1, 3, 5" or "all" or "none")
- Only after addressing their feedback (or if they choose "none") should you stop

Begin your review now.
EOF
    exit 2
fi

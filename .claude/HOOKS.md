# Claude Code Hooks Documentation

## Code Review Stop Hook

This project includes an automatic code review hook that triggers when Claude indicates it's done with a task.

### How It Works

1. **Trigger**: When Claude tries to stop/finish, the Stop hook runs automatically
2. **Check**: The hook script checks if there are any uncommitted changes in the git repository
3. **Action**:
   - If there are NO changes: Claude is allowed to stop normally
   - If there ARE changes: Claude is blocked from stopping and receives instructions to perform a code review

### Code Review Process

When code changes are detected, Claude will:

1. Review the changes made during the session (using memory, not re-reading files)
2. Identify issues and provide numbered findings, including:
   - Clear description of each issue
   - Explanation of why it's a concern
   - Suggestion for how to fix it
3. Ask you which changes you'd like to make
4. You can respond with:
   - Specific numbers: "1, 3, 5" to apply those changes
   - "all" to apply all suggested changes
   - "none" to skip all changes
5. After addressing your feedback, Claude will stop

### Files

- `.claude/settings.json` - Hook configuration
- `.claude/stop-hook-review.sh` - Shell script that checks for changes and triggers review

### Benefits

- **Automatic quality checks**: Never forget to review your code
- **Token-efficient**: Claude uses memory instead of re-reading files
- **Interactive**: You control which suggestions to apply
- **Non-intrusive**: Only triggers when code changes are detected

### Customization

You can modify the review criteria by editing `.claude/stop-hook-review.sh`. The script currently checks for:
- Modified files (git diff)
- Staged changes (git diff --cached)
- Untracked files (git ls-files --others --exclude-standard)

To disable the hook temporarily, you can either:
- Commit all changes before stopping
- Rename `.claude/settings.json` temporarily
- Modify the script to always `exit 0`

### Technical Details

- Hook type: Stop hook with command type
- Exit code 0: Allow stop (no changes detected)
- Exit code 2: Block stop and feed instructions to Claude (changes detected)
- Timeout: 10 seconds
- Input: Hook receives session info via stdin (session_id, cwd, etc.)

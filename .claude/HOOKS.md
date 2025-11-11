# Claude Code Hooks Documentation

## Code Review Stop Hook

This project includes an automatic code review hook that triggers when Claude indicates it's done with a task.

### How It Works

1. **Trigger**: When Claude tries to stop/finish, the Stop hook runs automatically
2. **Check**: The hook script checks if there are any uncommitted changes in the git repository
3. **Action**:
   - If there are NO changes: Claude is allowed to stop normally
   - If there ARE changes: Claude is instructed to use the `code-review` skill

### Code Review Skill

The actual review instructions are contained in a reusable skill at `.claude/skills/code-review/SKILL.md`. This skill:

- Reviews changes made during the session (using memory, not re-reading files)
- Analyzes code for correctness, performance, security, maintainability, and style
- Provides numbered findings with:
  - Clear description of each issue
  - Explanation of why it's a concern
  - Suggestion for how to fix it
- Asks which changes you'd like to make
- You can respond with:
  - Specific numbers: "1, 3, 5"
  - "all" to apply all suggestions
  - "none" to skip all changes

### Files

- `.claude/settings.json` - Hook configuration
- `.claude/stop-hook-review.sh` - Shell script that checks for changes
- `.claude/skills/code-review/SKILL.md` - Code review skill with detailed instructions
- `.claude/HOOKS.md` - This documentation

### Benefits

- **Automatic**: Triggers every time there are uncommitted changes
- **Token-efficient**: Claude uses memory instead of re-reading files
- **Interactive**: You control what gets fixed
- **Reusable**: The skill can be used independently with `/code-review` command
- **Maintainable**: Review instructions are centralized in the skill
- **Non-intrusive**: Only runs when code changes are detected

### Using the Code Review Skill Manually

You can also trigger a code review manually at any time by typing:
```
/code-review
```

This will invoke the skill without needing to stop first.

### Customization

#### Modify Review Criteria

Edit `.claude/skills/code-review/SKILL.md` to change:
- What aspects of code are reviewed
- How findings are formatted
- Priority and focus areas

#### Modify Trigger Conditions

Edit `.claude/stop-hook-review.sh` to change when the review triggers. Currently checks for:
- Modified files (git diff)
- Staged changes (git diff --cached)
- Untracked files (git ls-files --others --exclude-standard)

#### Disable Temporarily

To disable the hook temporarily:
- Commit all changes before stopping, or
- Rename `.claude/settings.json`, or
- Modify the script to always `exit 0`

### Technical Details

- Hook type: Stop hook with command type
- Exit code 0: Allow stop (no changes detected)
- Exit code 2: Block stop and instruct Claude to use skill (changes detected)
- Timeout: 10 seconds
- Skill location: `.claude/skills/code-review/SKILL.md` (project-level)
- Input: Hook receives session info via stdin (session_id, cwd, etc.)

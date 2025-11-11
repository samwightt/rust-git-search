# Claude Code Hooks Documentation

## Code Review Stop Hook

Automatic code review hook that triggers when Claude tries to stop.

### How It Works

1. **Trigger**: When Claude tries to stop/finish, the Stop hook runs
2. **Action**: Claude is instructed to use the `code-review` skill
3. **Review**: Claude reviews changes and provides numbered findings
4. **Your Choice**: Respond with which changes to apply (numbers, "all", or "none")

### Files

- `.claude/settings.json` - Hook configuration
- `.claude/stop-hook-review.sh` - Hook script
- `.claude/skills/code-review/SKILL.md` - Review skill with instructions and code style guidelines

### Manual Use

Invoke the code review skill anytime:
```
/code-review
```

### Customization

**Modify Review Criteria**: Edit `.claude/skills/code-review/SKILL.md`

**Disable Temporarily**: Rename `.claude/settings.json`

### Technical Details

- Hook type: Stop hook with command type
- Exit code 2: Always blocks and triggers skill
- Timeout: 10 seconds
- Skill location: `.claude/skills/code-review/SKILL.md`

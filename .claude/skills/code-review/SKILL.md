---
name: code-review
description: Performs a comprehensive code review of changes made during the current session. Use this skill when you've finished coding and need to review your work before stopping. Reviews code for correctness, performance, security, maintainability, and style issues.
---

# Code Review Skill

Review changes you made this session using your memory - **do NOT re-read files**.

## Review Criteria

Analyze for:
- **Correctness**: Logic errors, edge cases, bugs
- **Performance**: Inefficient algorithms, unnecessary operations
- **Security**: Vulnerabilities, input validation, sensitive data handling
- **Maintainability**: Code clarity, complexity
- **Style**: See code style guidelines below

## Code Style Guidelines

**Documentation and Comments**
- Use sparse inline comments - explain WHY, not WHAT
- Most functions need no comments if well-named
- Only use /// doc comments for public APIs or complex utility functions

**Error Handling**
- Use `anyhow::Result` as default for fallible functions
- Use `.expect("clear message")` for programming errors or truly unexpected states
- Use `.unwrap()` sparingly, only when error is genuinely impossible

**Code Organization**
- Public functions first, then private helpers, then tests at bottom
- Single blank line between functions/items
- Group related functionality together

**Functional Style**
- Prefer iterator chains (`.and_then()`, `.filter_map()`, `.collect()`) when readable
- Use pattern matching in function parameters when destructuring

**Naming**
- Clear, concise variable names - not overly verbose
- Match naming conventions of libraries you use

## Output Format

For each finding:

**[Number]. [Brief Title]**
- **Issue**: What's wrong or could improve
- **Why**: The concern
- **Fix**: Specific suggestion

## Guidelines

- Number each finding (1, 2, 3, etc.)
- Be concise and actionable
- Focus on meaningful issues
- Do NOT fix automatically
- Do NOT re-read files

## After Review

Ask which changes to make:
- Specific numbers: "1, 3, 5"
- "all" for all suggestions
- "none" to skip

Apply requested changes, then you may stop.

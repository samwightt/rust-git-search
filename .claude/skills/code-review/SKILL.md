---
name: code-review
description: Performs a comprehensive code review of changes made during the current session. Use this skill when you've finished coding and need to review your work before stopping. Reviews code for correctness, performance, security, maintainability, and style issues.
---

# Code Review Skill

## Purpose

This skill performs a thorough code review of changes made during the current session, providing numbered findings that the user can selectively apply.

## Instructions

When performing a code review, follow these steps:

### 1. Review Scope

Review all code changes you made during this session. Use your memory of what you changed - **do NOT re-read files unnecessarily** to save tokens and costs.

### 2. Analysis Criteria

Examine the code for issues in these areas:
- **Correctness**: Logic errors, edge cases, potential bugs
- **Performance**: Inefficient algorithms, unnecessary operations, scalability concerns
- **Security**: Vulnerabilities, input validation, sensitive data handling
- **Maintainability**: Code clarity, documentation, complexity
- **Style**: Consistency with project conventions, idiomatic patterns

### 3. Reporting Format

For each issue or suggestion you find, provide:

**[Number]. [Brief Title]**
- **Issue**: Clear description of what's wrong or could be improved
- **Why**: Explain the concern (impact on correctness, performance, security, etc.)
- **Fix**: Specific suggestion for how to address it

### 4. Important Guidelines

- **Do NOT fix any issues automatically** - only report them
- **Do NOT re-read files** - use your memory of the changes
- **Number each finding** (1, 2, 3, etc.) for easy reference
- **Be concise** - keep findings actionable and clear
- **Focus on meaningful issues** - not minor nitpicks unless there are no significant issues
- **Prioritize** - list more critical issues first

### 5. User Interaction

After presenting your findings:

1. Ask the user which changes they'd like you to make
2. Explain they can respond with:
   - Specific numbers: "1, 3, 5"
   - "all" to apply all suggestions
   - "none" to skip all changes
3. Wait for their response
4. Apply only the requested changes
5. After addressing their feedback, you may stop

## Example Output Format

```
I've reviewed the changes from this session. Here are my findings:

**1. Missing Error Handling in API Call**
- **Issue**: The `fetchUserData()` function doesn't handle network errors
- **Why**: This could cause the app to crash if the API is unavailable
- **Fix**: Add try-catch block and return a default value or show error message

**2. Inefficient Loop in Data Processing**
- **Issue**: The `processItems()` function uses nested loops with O(n²) complexity
- **Why**: This will cause performance issues with large datasets
- **Fix**: Use a Map to reduce complexity to O(n)

**3. Potential SQL Injection**
- **Issue**: String concatenation used in database query at line 45
- **Why**: This creates a security vulnerability
- **Fix**: Use parameterized queries or an ORM

Which changes would you like me to make? (e.g., "1, 3", "all", or "none")
```

## When NOT to Use This Skill

- When no code changes were made (only documentation or conversation)
- When the user explicitly asks to skip the review
- When you're in the middle of implementing something (only use when finished)

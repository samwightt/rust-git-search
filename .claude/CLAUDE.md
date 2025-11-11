# Claude Development Guide

## Git Hooks Setup

- This project uses lefthook for git hooks management
- If lefthook is not installed, install it using `go install github.com/evilmartians/lefthook@latest`
- After installing lefthook or when setting up the repository, run `lefthook install` to set up the git hooks
- The hooks will automatically run `cargo check` and `cargo clippy` on pre-commit, and `cargo test` on pre-push

## Development Workflow

- Use a TDD development workflow. This means implement one small bit at a time. Write a test for what you want to implement, run the module's tests to ensure it fails successfully, then implement only the logic necessary to make that test pass.
- Only write ONE test at a time. Do not write all of the tests at a time.
- Run cargo tests using the `--quiet` option to simplify output.

## Testing Guidelines

- Integration tests go in the `tests/` directory and should only test the high-level CLI API. They should not test implementation details.
- Unit tests go in the modules themselves using `#[cfg(test)] mod tests`.
- Use the test helpers found in `src/test_helpers.rs` for unit tests that need in-memory repositories.
- Use the test helpers in `tests/common/mod.rs` for integration tests that need actual git repositories.

## Code Style

### Documentation and Comments
- Use sparse inline comments - when used, comments explain WHY, not WHAT
- Most functions need no comments if well-named
- Only use `///` doc comments for public APIs or complex utility functions that truly need explanation

### Error Handling
- Use `anyhow::Result` as the default return type for functions that can fail
- Use `.expect("clear message")` when the error case indicates a programming error or truly unexpected state
- Use `.unwrap()` sparingly, only when the error is genuinely impossible

### Code Organization
- Public functions first, then private helpers, then tests at bottom
- Single blank line between functions/items
- Group related functionality together
- Keep module-level organization clean

### Functional Style
- Prefer iterator chains (`.and_then()`, `.filter_map()`, `.collect()`) over explicit loops when it improves readability
- Use pattern matching in function parameters when destructuring

### Naming
- Use clear, concise variable names - not overly verbose
- Match the naming conventions of the libraries you use

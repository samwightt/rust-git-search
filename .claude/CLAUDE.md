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

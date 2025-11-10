# Claude Development Guide

## Git Hooks Setup

- This project uses lefthook for git hooks management
- If lefthook is not installed, install it using `go install github.com/evilmartians/lefthook@latest`
- After installing lefthook or when setting up the repository, run `lefthook install` to set up the git hooks
- The hooks will automatically run `cargo check` and `cargo clippy` on pre-commit, and `cargo test` on pre-push

use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

/// Helper function to initialize a git repository with test commits
pub fn setup_test_repo() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let repo_path = temp_dir.path().to_path_buf();

    // Initialize git repo
    let output = std::process::Command::new("git")
        .arg("init")
        .current_dir(&repo_path)
        .output()
        .expect("Failed to init git repo");

    if !output.status.success() {
        panic!("git init failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    // Set initial branch name
    let output = std::process::Command::new("git")
        .args(["config", "init.defaultBranch", "main"])
        .current_dir(&repo_path)
        .output()
        .expect("Failed to set default branch");

    if !output.status.success() {
        panic!("git config init.defaultBranch failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    // Configure git user for commits
    let output = std::process::Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(&repo_path)
        .output()
        .expect("Failed to configure git user.name");

    if !output.status.success() {
        panic!("git config user.name failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    let output = std::process::Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(&repo_path)
        .output()
        .expect("Failed to configure git user.email");

    if !output.status.success() {
        panic!("git config user.email failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    // Disable commit signing for tests
    let output = std::process::Command::new("git")
        .args(["config", "commit.gpgsign", "false"])
        .current_dir(&repo_path)
        .output()
        .expect("Failed to configure git commit.gpgsign");

    if !output.status.success() {
        panic!("git config commit.gpgsign failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    (temp_dir, repo_path)
}

/// Helper to create a file and commit it
pub fn create_and_commit_file(repo_path: &PathBuf, filename: &str, content: &str, commit_msg: &str) {
    let file_path = repo_path.join(filename);
    fs::write(&file_path, content).expect("Failed to write file");

    let output = std::process::Command::new("git")
        .args(["add", filename])
        .current_dir(repo_path)
        .output()
        .expect("Failed to git add");

    if !output.status.success() {
        panic!("git add failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    let output = std::process::Command::new("git")
        .args(["commit", "-m", commit_msg])
        .current_dir(repo_path)
        .output()
        .expect("Failed to git commit");

    if !output.status.success() {
        panic!("git commit failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    // Sleep to ensure distinct commit timestamps
    thread::sleep(Duration::from_millis(1100));
}

/// Helper to delete a file and commit it
pub fn delete_and_commit_file(repo_path: &PathBuf, filename: &str, commit_msg: &str) {
    let output = std::process::Command::new("git")
        .args(["rm", filename])
        .current_dir(repo_path)
        .output()
        .expect("Failed to git rm");

    if !output.status.success() {
        panic!("git rm failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    let output = std::process::Command::new("git")
        .args(["commit", "-m", commit_msg])
        .current_dir(repo_path)
        .output()
        .expect("Failed to git commit");

    if !output.status.success() {
        panic!("git commit failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    // Sleep to ensure distinct commit timestamps
    thread::sleep(Duration::from_millis(1100));
}

/// Helper to create/modify multiple files and commit them together
pub fn create_and_commit_files(repo_path: &PathBuf, files: &[(&str, &str)], commit_msg: &str) {
    for (filename, content) in files {
        let file_path = repo_path.join(filename);

        // Create parent directories if needed
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).expect("Failed to create parent directories");
        }

        fs::write(&file_path, content).expect("Failed to write file");

        let output = std::process::Command::new("git")
            .args(["add", filename])
            .current_dir(repo_path)
            .output()
            .expect("Failed to git add");

        if !output.status.success() {
            panic!("git add failed: {}", String::from_utf8_lossy(&output.stderr));
        }
    }

    let output = std::process::Command::new("git")
        .args(["commit", "-m", commit_msg])
        .current_dir(repo_path)
        .output()
        .expect("Failed to git commit");

    if !output.status.success() {
        panic!("git commit failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    // Sleep to ensure distinct commit timestamps
    thread::sleep(Duration::from_millis(1100));
}

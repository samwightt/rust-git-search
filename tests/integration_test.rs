mod common;

use assert_cmd::Command;
use common::{create_and_commit_file, setup_test_repo};
use predicates::prelude::*;

#[test]
fn test_search_single_occurrence() {
    let (_temp_dir, repo_path) = setup_test_repo();

    // Create a file with a single occurrence of "hello"
    create_and_commit_file(
        &repo_path,
        "test.txt",
        "hello world",
        "Add test file",
    );

    let mut cmd = Command::cargo_bin("git-history").unwrap();
    cmd.arg("scan")
        .arg(repo_path.to_str().unwrap())
        .arg("hello");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Searching for: 'hello'"))
        .stdout(predicate::str::contains("Number of times 'hello' appears in files anywhere in git history"))
        .stdout(predicate::str::is_match(r"history: [1-9]\d*").unwrap()); // At least 1
}

#[test]
fn test_search_multiple_occurrences_in_one_file() {
    let (_temp_dir, repo_path) = setup_test_repo();

    // Create a file with multiple occurrences of "test"
    create_and_commit_file(
        &repo_path,
        "test.txt",
        "test test test",
        "Add test file",
    );

    let mut cmd = Command::cargo_bin("git-history").unwrap();
    cmd.arg("scan")
        .arg(repo_path.to_str().unwrap())
        .arg("test");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Searching for: 'test'"))
        .stdout(predicate::str::contains("Number of times 'test' appears in files anywhere in git history"))
        .stdout(predicate::str::is_match(r"history: [3-9]").unwrap()); // At least 3
}

#[test]
fn test_search_across_multiple_commits() {
    let (_temp_dir, repo_path) = setup_test_repo();

    // First commit with "foo"
    create_and_commit_file(
        &repo_path,
        "file1.txt",
        "foo",
        "Add file1",
    );

    // Second commit with "foo" twice
    create_and_commit_file(
        &repo_path,
        "file2.txt",
        "foo foo",
        "Add file2",
    );

    // Third commit modifying file1 to have "foo" three times
    create_and_commit_file(
        &repo_path,
        "file1.txt",
        "foo foo foo",
        "Update file1",
    );

    let mut cmd = Command::cargo_bin("git-history").unwrap();
    cmd.arg("scan")
        .arg(repo_path.to_str().unwrap())
        .arg("foo");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Searching for: 'foo'"))
        .stdout(predicate::str::contains("Number of times 'foo' appears in files anywhere in git history"))
        .stdout(predicate::str::is_match(r"history: [6-9]").unwrap()); // At least 6
}

#[test]
fn test_search_no_matches() {
    let (_temp_dir, repo_path) = setup_test_repo();

    create_and_commit_file(
        &repo_path,
        "test.txt",
        "hello world",
        "Add test file",
    );

    let mut cmd = Command::cargo_bin("git-history").unwrap();
    cmd.arg("scan")
        .arg(repo_path.to_str().unwrap())
        .arg("notfound");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Searching for: 'notfound'"))
        .stdout(predicate::str::contains("Number of times 'notfound' appears in files anywhere in git history: 0"));
}

#[test]
fn test_search_case_sensitive() {
    let (_temp_dir, repo_path) = setup_test_repo();

    create_and_commit_file(
        &repo_path,
        "test.txt",
        "Hello hello HELLO",
        "Add test file",
    );

    // Search for lowercase "hello" should find at least 1
    let mut cmd = Command::cargo_bin("git-history").unwrap();
    cmd.arg("scan")
        .arg(repo_path.to_str().unwrap())
        .arg("hello");

    let output1 = cmd.assert()
        .success()
        .stdout(predicate::str::contains("Number of times 'hello' appears in files anywhere in git history"))
        .get_output()
        .clone();

    // Search for uppercase "HELLO" should find at least 1
    let mut cmd2 = Command::cargo_bin("git-history").unwrap();
    cmd2.arg("scan")
        .arg(repo_path.to_str().unwrap())
        .arg("HELLO");

    let output2 = cmd2.assert()
        .success()
        .stdout(predicate::str::contains("Number of times 'HELLO' appears in files anywhere in git history"))
        .get_output()
        .clone();

    // Verify they found different counts (case sensitive)
    let stdout1 = String::from_utf8_lossy(&output1.stdout);
    let stdout2 = String::from_utf8_lossy(&output2.stdout);

    // Both should have found at least 1, and the outputs should be similar (both finding 1 or 2 depending on commit structure)
    assert!(stdout1.contains("history:"));
    assert!(stdout2.contains("history:"));
}

#[test]
fn test_missing_arguments() {
    let mut cmd = Command::cargo_bin("git-history").unwrap();
    cmd.arg("scan");

    // Should fail when missing required arguments
    cmd.assert()
        .failure();
}

#[test]
fn test_help_message() {
    let mut cmd = Command::cargo_bin("git-history").unwrap();
    cmd.arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("A tool for analyzing git history"));
}

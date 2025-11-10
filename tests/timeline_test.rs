mod common;

use assert_cmd::Command;
use common::{create_and_commit_file, delete_and_commit_file, setup_test_repo};
use std::fs;

#[test]
fn test_timeline_addition_delta() {
    let (_temp_dir, repo_path) = setup_test_repo();

    // First commit: add a file with 3 occurrences of "test"
    create_and_commit_file(
        &repo_path,
        "file1.txt",
        "test test test",
        "Add file1 with test",
    );

    let output_file = repo_path.join("timeline.jsonl");

    let mut cmd = Command::cargo_bin("git-history").unwrap();
    cmd.arg("timeline")
        .arg(repo_path.to_str().unwrap())
        .arg("test")
        .arg("--output")
        .arg(output_file.to_str().unwrap());

    cmd.assert().success();

    // Read the output file
    let content = fs::read_to_string(&output_file).expect("Failed to read output file");
    let lines: Vec<&str> = content.lines().collect();

    assert_eq!(lines.len(), 1, "Should have 1 commit in timeline");

    // Parse the JSON line
    let json: serde_json::Value = serde_json::from_str(lines[0]).expect("Failed to parse JSON");

    // Verify the count is 3 (3 occurrences added)
    assert_eq!(json["count"].as_i64().unwrap(), 3);
    assert!(json["commit_id"].is_string());
    assert!(json["date"].is_string());
    assert!(json["message"].as_str().unwrap().contains("Add file1 with test"));
    assert_eq!(json["author_name"].as_str().unwrap(), "Test User");
    assert_eq!(json["author_email"].as_str().unwrap(), "test@example.com");
}

#[test]
fn test_timeline_deletion_delta() {
    let (_temp_dir, repo_path) = setup_test_repo();

    // First commit: add a file with 5 occurrences of "foo"
    create_and_commit_file(
        &repo_path,
        "file1.txt",
        "foo foo foo foo foo",
        "Add file1",
    );

    // Second commit: delete the file
    delete_and_commit_file(
        &repo_path,
        "file1.txt",
        "Delete file1",
    );

    let output_file = repo_path.join("timeline.jsonl");

    let mut cmd = Command::cargo_bin("git-history").unwrap();
    cmd.arg("timeline")
        .arg(repo_path.to_str().unwrap())
        .arg("foo")
        .arg("--output")
        .arg(output_file.to_str().unwrap());

    cmd.assert().success();

    // Read the output file
    let content = fs::read_to_string(&output_file).expect("Failed to read output file");
    let lines: Vec<&str> = content.lines().collect();

    assert_eq!(lines.len(), 2, "Should have 2 commits in timeline");

    // Parse the JSON lines
    let json1: serde_json::Value = serde_json::from_str(lines[0]).expect("Failed to parse JSON line 1");
    let json2: serde_json::Value = serde_json::from_str(lines[1]).expect("Failed to parse JSON line 2");

    // First commit: count should be 5 (added 5 occurrences)
    assert_eq!(json1["count"].as_i64().unwrap(), 5);
    assert!(json1["message"].as_str().unwrap().contains("Add file1"));

    // Second commit: count should be 0 (deleted 5 occurrences, so 5 - 5 = 0)
    assert_eq!(json2["count"].as_i64().unwrap(), 0);
    assert!(json2["message"].as_str().unwrap().contains("Delete file1"));
}

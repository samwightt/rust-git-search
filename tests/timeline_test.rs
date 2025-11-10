mod common;

use assert_cmd::Command;
use common::{create_and_commit_file, create_and_commit_files, delete_and_commit_file, setup_test_repo};
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

#[test]
fn test_timeline_modification_delta() {
    let (_temp_dir, repo_path) = setup_test_repo();

    // First commit: add a file with 2 occurrences of "bar"
    create_and_commit_file(
        &repo_path,
        "file1.txt",
        "bar bar",
        "Add file1 with 2 bars",
    );

    // Second commit: modify the file to have 5 occurrences
    create_and_commit_file(
        &repo_path,
        "file1.txt",
        "bar bar bar bar bar",
        "Update file1 to 5 bars",
    );

    let output_file = repo_path.join("timeline.jsonl");

    let mut cmd = Command::cargo_bin("git-history").unwrap();
    cmd.arg("timeline")
        .arg(repo_path.to_str().unwrap())
        .arg("bar")
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

    // First commit: count should be 2 (added 2 occurrences)
    assert_eq!(json1["count"].as_i64().unwrap(), 2);
    assert!(json1["message"].as_str().unwrap().contains("Add file1 with 2 bars"));

    // Second commit: count should be 5 (was 2, added 3 more, so 2 + 3 = 5)
    assert_eq!(json2["count"].as_i64().unwrap(), 5);
    assert!(json2["message"].as_str().unwrap().contains("Update file1 to 5 bars"));
}

#[test]
fn test_timeline_multiple_changes_in_one_commit() {
    let (_temp_dir, repo_path) = setup_test_repo();

    // First commit: add file1 with 3 occurrences and file2 with 2 occurrences
    create_and_commit_files(
        &repo_path,
        &[
            ("file1.txt", "baz baz baz"),
            ("file2.txt", "baz baz"),
        ],
        "Add file1 and file2",
    );

    // Second commit: modify both files
    // file1: 3 -> 1 (delta: -2)
    // file2: 2 -> 4 (delta: +2)
    // Total delta: 0, so count should remain 5
    create_and_commit_files(
        &repo_path,
        &[
            ("file1.txt", "baz"),
            ("file2.txt", "baz baz baz baz"),
        ],
        "Update both files",
    );

    let output_file = repo_path.join("timeline.jsonl");

    let mut cmd = Command::cargo_bin("git-history").unwrap();
    cmd.arg("timeline")
        .arg(repo_path.to_str().unwrap())
        .arg("baz")
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

    // First commit: count should be 5 (3 + 2)
    assert_eq!(json1["count"].as_i64().unwrap(), 5);
    assert!(json1["message"].as_str().unwrap().contains("Add file1 and file2"));

    // Second commit: count should still be 5 (5 + (-2) + 2 = 5)
    assert_eq!(json2["count"].as_i64().unwrap(), 5);
    assert!(json2["message"].as_str().unwrap().contains("Update both files"));
}

#[test]
fn test_timeline_default_output_filename() {
    let (_temp_dir, repo_path) = setup_test_repo();

    // Create a commit
    create_and_commit_file(
        &repo_path,
        "file1.txt",
        "qux qux",
        "Add file1",
    );

    // Run timeline without --output flag (should use default filename in current dir)
    let mut cmd = Command::cargo_bin("git-history").unwrap();
    cmd.arg("timeline")
        .arg(repo_path.to_str().unwrap())
        .arg("qux")
        .current_dir(&repo_path);  // Run from repo directory so default file appears there

    cmd.assert().success();

    // Check that the default file was created in the repo directory
    let default_file = repo_path.join("timeline.jsonl");
    assert!(default_file.exists(), "Default output file should exist at {:?}", default_file);

    // Verify the content
    let content = fs::read_to_string(&default_file).expect("Failed to read default output file");
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 1, "Should have 1 commit in timeline");

    let json: serde_json::Value = serde_json::from_str(lines[0]).expect("Failed to parse JSON");
    assert_eq!(json["count"].as_i64().unwrap(), 2);
}

#[test]
fn test_timeline_metadata_validation() {
    let (_temp_dir, repo_path) = setup_test_repo();

    create_and_commit_file(
        &repo_path,
        "file1.txt",
        "metadata test",
        "Test commit message",
    );

    let output_file = repo_path.join("timeline.jsonl");

    let mut cmd = Command::cargo_bin("git-history").unwrap();
    cmd.arg("timeline")
        .arg(repo_path.to_str().unwrap())
        .arg("metadata")
        .arg("--output")
        .arg(output_file.to_str().unwrap());

    cmd.assert().success();

    let content = fs::read_to_string(&output_file).expect("Failed to read output file");
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 1);

    let json: serde_json::Value = serde_json::from_str(lines[0]).expect("Failed to parse JSON");

    // Validate all required fields exist
    assert!(json["date"].is_string(), "date field should be a string");
    assert!(json["commit_id"].is_string(), "commit_id field should be a string");
    assert!(json["message"].is_string(), "message field should be a string");
    assert!(json["author_name"].is_string(), "author_name field should be a string");
    assert!(json["author_email"].is_string(), "author_email field should be a string");
    assert!(json["count"].is_number(), "count field should be a number");

    // Validate ISO 8601 date format (should contain 'T' and timezone info)
    let date_str = json["date"].as_str().unwrap();
    assert!(date_str.contains('T'), "Date should be in ISO 8601 format with 'T' separator");
    assert!(date_str.contains('+') || date_str.contains('Z') || date_str.contains('-'),
            "Date should include timezone information");

    // Validate commit_id is a valid hex string (40 chars for SHA-1)
    let commit_id = json["commit_id"].as_str().unwrap();
    assert_eq!(commit_id.len(), 40, "Commit ID should be 40 characters (SHA-1)");
    assert!(commit_id.chars().all(|c| c.is_ascii_hexdigit()), "Commit ID should be hex");

    // Validate metadata values match our test data
    assert_eq!(json["message"].as_str().unwrap(), "Test commit message");
    assert_eq!(json["author_name"].as_str().unwrap(), "Test User");
    assert_eq!(json["author_email"].as_str().unwrap(), "test@example.com");
    assert_eq!(json["count"].as_i64().unwrap(), 1);
}

#[test]
fn test_timeline_no_matches_in_history() {
    let (_temp_dir, repo_path) = setup_test_repo();

    // Create multiple commits with content that doesn't match the search string
    create_and_commit_file(
        &repo_path,
        "file1.txt",
        "hello world",
        "Add file1",
    );

    create_and_commit_file(
        &repo_path,
        "file2.txt",
        "foo bar baz",
        "Add file2",
    );

    create_and_commit_file(
        &repo_path,
        "file3.txt",
        "lorem ipsum",
        "Add file3",
    );

    let output_file = repo_path.join("timeline.jsonl");

    let mut cmd = Command::cargo_bin("git-history").unwrap();
    cmd.arg("timeline")
        .arg(repo_path.to_str().unwrap())
        .arg("NOMATCH")  // String that doesn't appear in any file
        .arg("--output")
        .arg(output_file.to_str().unwrap());

    cmd.assert().success();

    let content = fs::read_to_string(&output_file).expect("Failed to read output file");
    let lines: Vec<&str> = content.lines().collect();

    // Should have 3 commits
    assert_eq!(lines.len(), 3, "Should have 3 commits in timeline");

    // All commits should have count=0
    for (i, line) in lines.iter().enumerate() {
        let json: serde_json::Value = serde_json::from_str(line)
            .expect(&format!("Failed to parse JSON line {}", i));
        assert_eq!(json["count"].as_i64().unwrap(), 0,
                   "Commit {} should have count=0 when no matches found", i);
    }
}

#[test]
fn test_timeline_case_sensitive_search() {
    let (_temp_dir, repo_path) = setup_test_repo();

    // Create commits with mixed case content
    create_and_commit_file(
        &repo_path,
        "file1.txt",
        "Test test TEST",
        "Add mixed case file",
    );

    let output_file = repo_path.join("timeline.jsonl");

    // Search for lowercase "test"
    let mut cmd = Command::cargo_bin("git-history").unwrap();
    cmd.arg("timeline")
        .arg(repo_path.to_str().unwrap())
        .arg("test")
        .arg("--output")
        .arg(output_file.to_str().unwrap());

    cmd.assert().success();

    let content = fs::read_to_string(&output_file).expect("Failed to read output file");
    let lines: Vec<&str> = content.lines().collect();
    let json: serde_json::Value = serde_json::from_str(lines[0]).expect("Failed to parse JSON");

    // Should only match lowercase "test", not "Test" or "TEST"
    assert_eq!(json["count"].as_i64().unwrap(), 1, "Should find exactly 1 lowercase 'test'");

    // Now search for uppercase "TEST"
    let output_file2 = repo_path.join("timeline2.jsonl");
    let mut cmd2 = Command::cargo_bin("git-history").unwrap();
    cmd2.arg("timeline")
        .arg(repo_path.to_str().unwrap())
        .arg("TEST")
        .arg("--output")
        .arg(output_file2.to_str().unwrap());

    cmd2.assert().success();

    let content2 = fs::read_to_string(&output_file2).expect("Failed to read output file");
    let lines2: Vec<&str> = content2.lines().collect();
    let json2: serde_json::Value = serde_json::from_str(lines2[0]).expect("Failed to parse JSON");

    // Should only match uppercase "TEST"
    assert_eq!(json2["count"].as_i64().unwrap(), 1, "Should find exactly 1 uppercase 'TEST'");
}

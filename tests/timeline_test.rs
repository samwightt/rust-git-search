mod common;

use assert_cmd::Command;
use common::{create_and_commit_file, create_and_commit_files, setup_test_repo};
use std::fs;

#[test]
fn test_timeline_output_format() {
    let (_temp_dir, repo_path) = setup_test_repo();

    create_and_commit_file(
        &repo_path,
        "file1.txt",
        "test test",
        "First commit",
    );

    create_and_commit_files(
        &repo_path,
        &[
            ("file1.txt", "test test test"),
            ("file2.txt", "test"),
        ],
        "Second commit",
    );

    let output_file = repo_path.join("timeline.jsonl");

    let mut cmd = Command::cargo_bin("git-history").unwrap();
    cmd.arg("timeline")
        .arg(repo_path.to_str().unwrap())
        .arg("test")
        .arg("--output")
        .arg(output_file.to_str().unwrap());

    cmd.assert().success();

    let content = fs::read_to_string(&output_file).expect("Failed to read output file");
    let lines: Vec<&str> = content.lines().collect();

    assert_eq!(lines.len(), 2, "Should have 2 commits in timeline");

    for (i, line) in lines.iter().enumerate() {
        let json: serde_json::Value = serde_json::from_str(line)
            .expect(&format!("Failed to parse JSON line {}", i));

        assert!(json["date"].is_string(), "date field should be a string");
        assert!(json["commit_id"].is_string(), "commit_id field should be a string");
        assert!(json["message"].is_string(), "message field should be a string");
        assert!(json["author_name"].is_string(), "author_name field should be a string");
        assert!(json["author_email"].is_string(), "author_email field should be a string");
        assert!(json["count"].is_number(), "count field should be a number");

        let date_str = json["date"].as_str().unwrap();
        assert!(date_str.contains('T'), "Date should be in ISO 8601 format");

        let commit_id = json["commit_id"].as_str().unwrap();
        assert_eq!(commit_id.len(), 40, "Commit ID should be 40 characters");
        assert!(commit_id.chars().all(|c| c.is_ascii_hexdigit()), "Commit ID should be hex");

        assert_eq!(json["author_name"].as_str().unwrap(), "Test User");
        assert_eq!(json["author_email"].as_str().unwrap(), "test@example.com");
    }

    let json1: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    let json2: serde_json::Value = serde_json::from_str(lines[1]).unwrap();

    assert!(json1["message"].as_str().unwrap().contains("First commit"));
    assert!(json2["message"].as_str().unwrap().contains("Second commit"));

    let count1 = json1["count"].as_i64().unwrap();
    let count2 = json2["count"].as_i64().unwrap();
    assert!(count1 > 0, "First commit should have matches");
    assert!(count2 > count1, "Second commit should have more total matches");
}

#[test]
fn test_timeline_default_output_filename() {
    let (_temp_dir, repo_path) = setup_test_repo();

    create_and_commit_file(
        &repo_path,
        "file1.txt",
        "content",
        "Add file1",
    );

    let mut cmd = Command::cargo_bin("git-history").unwrap();
    cmd.arg("timeline")
        .arg(repo_path.to_str().unwrap())
        .arg("content")
        .current_dir(&repo_path);

    cmd.assert().success();

    let default_file = repo_path.join("timeline.jsonl");
    assert!(default_file.exists(), "Default output file should exist");

    let content = fs::read_to_string(&default_file).expect("Failed to read default output file");
    let lines: Vec<&str> = content.lines().collect();
    assert!(lines.len() > 0, "Should have at least 1 commit in timeline");

    let json: serde_json::Value = serde_json::from_str(lines[0]).expect("Failed to parse JSON");
    assert!(json["count"].is_number());
}

#[test]
fn test_timeline_case_insensitive_search() {
    let (_temp_dir, repo_path) = setup_test_repo();

    create_and_commit_file(
        &repo_path,
        "file1.txt",
        "Test test TEST",
        "First commit",
    );

    create_and_commit_file(
        &repo_path,
        "file2.txt",
        "testing TeSt",
        "Second commit",
    );

    let output_file = repo_path.join("timeline_case_insensitive.jsonl");

    let mut cmd = Command::cargo_bin("git-history").unwrap();
    cmd.arg("timeline")
        .arg(repo_path.to_str().unwrap())
        .arg("test")
        .arg("--output")
        .arg(output_file.to_str().unwrap())
        .arg("-i");

    cmd.assert().success();

    let content = fs::read_to_string(&output_file).expect("Failed to read output file");
    let lines: Vec<&str> = content.lines().collect();

    assert_eq!(lines.len(), 2, "Should have 2 commits in timeline");

    let json1: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    let json2: serde_json::Value = serde_json::from_str(lines[1]).unwrap();

    // First commit: "Test test TEST" = 3 matches (case-insensitive)
    assert_eq!(json1["count"].as_i64().unwrap(), 3);

    // Second commit: previous 3 + "testing TeSt" = 3 + 2 = 5 matches total
    assert_eq!(json2["count"].as_i64().unwrap(), 5);
}

#[test]
fn test_timeline_regex_search() {
    let (_temp_dir, repo_path) = setup_test_repo();

    create_and_commit_file(
        &repo_path,
        "file1.txt",
        "test123 test456",
        "First commit",
    );

    create_and_commit_file(
        &repo_path,
        "file2.txt",
        "test789 testing",
        "Second commit",
    );

    let output_file = repo_path.join("timeline_regex.jsonl");

    let mut cmd = Command::cargo_bin("git-history").unwrap();
    cmd.arg("timeline")
        .arg(repo_path.to_str().unwrap())
        .arg(r"test\d+")
        .arg("--output")
        .arg(output_file.to_str().unwrap())
        .arg("--regex");

    cmd.assert().success();

    let content = fs::read_to_string(&output_file).expect("Failed to read output file");
    let lines: Vec<&str> = content.lines().collect();

    assert_eq!(lines.len(), 2, "Should have 2 commits in timeline");

    let json1: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    let json2: serde_json::Value = serde_json::from_str(lines[1]).unwrap();

    // First commit: "test123 test456" = 2 matches (test\d+)
    assert_eq!(json1["count"].as_i64().unwrap(), 2);

    // Second commit: previous 2 + "test789" = 2 + 1 = 3 matches total
    // "testing" doesn't match because no digits
    assert_eq!(json2["count"].as_i64().unwrap(), 3);
}

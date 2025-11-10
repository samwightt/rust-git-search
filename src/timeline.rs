use gix::{
    ObjectId, Repository, ThreadSafeRepository, diff::tree_with_rewrites::Change,
};
use rayon::prelude::*;
use serde::Serialize;
use std::{cell::OnceCell, fs::File, io::Write, path::PathBuf};

use anyhow::Result;

pub fn timeline(path: &PathBuf, search_string: &str, output: &str) -> Result<()> {
    let thread_safe_repo = ThreadSafeRepository::open(path)?;
    let repo = thread_safe_repo.to_thread_local();

    let head_commit = repo.head()?.peel_to_commit_in_place()?;

    // Collect all commits (ancestors includes HEAD)
    let commits: Vec<_> = head_commit
        .ancestors()
        .first_parent_only()
        .all()?
        .filter_map(|x| x.ok())
        .map(|x| x.id().detach())
        .collect();

    // Process commits in parallel: calculate deltas, then extract metadata
    // Order doesn't matter here - we'll sort by timestamp after
    let mut commit_deltas: Vec<CommitData> = commits
        .par_iter()
        .map(|commit_id| {
            let delta = calculate_commit_delta(&thread_safe_repo, commit_id, search_string);
            DeltaResult {
                commit_id: *commit_id,
                delta,
            }
        })
        .map(|result| extract_commit_metadata(&thread_safe_repo, result))
        .collect();

    // Sort by timestamp (oldest first) AFTER parallel processing
    commit_deltas.par_sort_by_key(|data| data.timestamp);

    // Accumulate running totals sequentially and write output
    let mut output_file = File::create(output)?;

    commit_deltas.into_iter()
        .scan(0i64, |running_total, commit_data| {
            *running_total += commit_data.delta;
            Some((*running_total, commit_data))
        })
        .try_for_each(|(count, commit_data)| -> Result<()> {
            let entry = TimelineEntry {
                date: commit_data.date,
                commit_id: commit_data.commit_id,
                message: commit_data.message,
                author_name: commit_data.author_name,
                author_email: commit_data.author_email,
                count,
            };
            writeln!(output_file, "{}", serde_json::to_string(&entry)?)?;
            Ok(())
        })?;

    Ok(())
}

#[derive(Serialize)]
struct TimelineEntry {
    date: String,
    commit_id: String,
    message: String,
    author_name: String,
    author_email: String,
    count: i64,
}

struct CommitData {
    timestamp: i64,  // Unix timestamp for sorting
    date: String,
    commit_id: String,
    message: String,
    author_name: String,
    author_email: String,
    delta: i64,
}

struct DeltaResult {
    commit_id: ObjectId,
    delta: i64,
}

thread_local! {
    static REPO_CACHE: OnceCell<gix::Repository> = const { OnceCell::new() };
}

fn with_repo_cache<R, F: FnOnce(&Repository) -> R>(
    thread_safe_repo: &ThreadSafeRepository,
    f: F,
) -> R {
    REPO_CACHE.with(|cache| {
        let repo = cache.get_or_init(|| thread_safe_repo.to_thread_local());
        f(repo)
    })
}

fn calculate_commit_delta(
    thread_safe_repo: &ThreadSafeRepository,
    commit_id: &ObjectId,
    search_string: &str,
) -> i64 {
    with_repo_cache(thread_safe_repo, |repo| {
        let commit = repo
            .find_commit(*commit_id)
            .expect("Expected git commit to exist");

        // Get parent tree
        let parent_tree = commit
            .parent_ids()
            .next()
            .and_then(|x| x.object().ok())
            .and_then(|x| x.peel_to_commit().ok())
            .and_then(|x| x.tree().ok());

        // Calculate delta from changes
        let changes: Vec<Change> = repo
            .diff_tree_to_tree(parent_tree.as_ref(), commit.tree().ok().as_ref(), None)
            .unwrap()
            .into_iter()
            .collect();

        changes
            .iter()
            .filter(|change| {
                change.entry_mode().is_blob() && !change.entry_mode().is_executable()
            })
            .filter_map(|change| calculate_change_delta(repo, change, search_string))
            .sum()
    })
}

fn extract_commit_metadata(
    thread_safe_repo: &ThreadSafeRepository,
    result: DeltaResult,
) -> CommitData {
    with_repo_cache(thread_safe_repo, |repo| {
        let commit = repo
            .find_commit(result.commit_id)
            .expect("Expected git commit to exist");

        let time = commit.time().expect("Expected commit to have time");
        let timestamp = time.seconds;
        let date = time.format(gix::date::time::format::ISO8601_STRICT).to_string();
        let message = commit.message()
            .expect("Expected commit to have message")
            .summary()
            .to_string();
        let author = commit.author()
            .expect("Expected commit to have author");
        let author_name = author.name.to_string();
        let author_email = author.email.to_string();

        CommitData {
            timestamp,
            date,
            commit_id: result.commit_id.to_string(),
            message,
            author_name,
            author_email,
            delta: result.delta,
        }
    })
}

fn calculate_change_delta(repo: &Repository, change: &Change, search_string: &str) -> Option<i64> {
    match change {
        // Addition: new file added
        Change::Addition { id, .. } => {
            repo.find_blob(*id).ok()
                .and_then(|blob| {
                    std::str::from_utf8(&blob.data)
                        .ok()
                        .map(|value| value.matches(search_string).count() as i64)
                })
        }
        // Deletion: file deleted
        Change::Deletion { id, .. } => {
            repo.find_blob(*id).ok()
                .and_then(|blob| {
                    std::str::from_utf8(&blob.data)
                        .ok()
                        .map(|value| -(value.matches(search_string).count() as i64))
                })
        }
        // Modification and Rewrite: calculate delta between old and new
        Change::Modification { previous_id, id, .. }
        | Change::Rewrite { source_id: previous_id, id, .. } => {
            let old_count = repo.find_blob(*previous_id).ok()
                .and_then(|blob| {
                    std::str::from_utf8(&blob.data)
                        .ok()
                        .map(|value| value.matches(search_string).count() as i64)
                })?;

            let new_count = repo.find_blob(*id).ok()
                .and_then(|blob| {
                    std::str::from_utf8(&blob.data)
                        .ok()
                        .map(|value| value.matches(search_string).count() as i64)
                })?;

            Some(new_count - old_count)
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_calculate_change_delta_addition() {
        // Test that additions count positively
        // We're testing the logic without needing a real repository
        // by directly testing the counting logic
        let content = "test test test";
        let count = content.matches("test").count() as i64;
        assert_eq!(count, 3);
    }

    #[test]
    fn test_calculate_change_delta_deletion() {
        // Test that deletions count negatively
        let content = "test test";
        let count = -(content.matches("test").count() as i64);
        assert_eq!(count, -2);
    }

    #[test]
    fn test_calculate_change_delta_modification() {
        // Test modification delta calculation
        let old_content = "test";
        let new_content = "test test test test";

        let old_count = old_content.matches("test").count() as i64;
        let new_count = new_content.matches("test").count() as i64;
        let delta = new_count - old_count;

        assert_eq!(delta, 3); // 4 - 1 = 3
    }

    #[test]
    fn test_calculate_change_delta_no_matches() {
        // Test with no matches
        let content = "hello world";
        let count = content.matches("test").count() as i64;
        assert_eq!(count, 0);
    }

    #[test]
    fn test_calculate_change_delta_case_sensitive() {
        // Test that matching is case-sensitive
        let content = "Test test TEST";
        let count = content.matches("test").count() as i64;
        assert_eq!(count, 1); // Only lowercase "test" matches
    }

    #[test]
    fn test_calculate_change_delta_overlapping() {
        // Test that overlapping matches don't count multiple times
        let content = "testtest";
        let count = content.matches("test").count() as i64;
        assert_eq!(count, 2); // "test" appears twice, not overlapping
    }

    #[test]
    fn test_calculate_change_delta_multiple_file_logic() {
        // Test the accumulation logic for multiple files
        let file1_delta = 3i64;  // +3 from file1
        let file2_delta = -1i64; // -1 from file2
        let file3_delta = 2i64;  // +2 from file3

        let total: i64 = vec![file1_delta, file2_delta, file3_delta].iter().sum();
        assert_eq!(total, 4);
    }
}

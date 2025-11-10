use gix::{
    ObjectId, Repository, ThreadSafeRepository, diff::tree_with_rewrites::Change,
};
use rayon::prelude::*;
use serde::Serialize;
use std::{cell::OnceCell, fs::File, io::Write, path::Path};

use anyhow::Result;

pub fn timeline(path: &Path, search_string: &str, output: &str) -> Result<()> {
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
        repo
            .diff_tree_to_tree(parent_tree.as_ref(), commit.tree().ok().as_ref(), None)
            .unwrap()
            .into_iter()
            .filter(|change| {
                change.entry_mode().is_blob() && !change.entry_mode().is_executable()
            })
            .filter_map(|change| calculate_change_delta(repo, &change, search_string))
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

fn count_matches_in_blob(repo: &Repository, id: ObjectId, search_string: &str) -> Option<i64> {
    repo.find_blob(id).ok()
        .and_then(|blob| {
            std::str::from_utf8(&blob.data)
                .ok()
                .map(|value| value.matches(search_string).count() as i64)
        })
}

fn calculate_change_delta(repo: &Repository, change: &Change, search_string: &str) -> Option<i64> {
    match change {
        Change::Addition { id, .. } => {
            count_matches_in_blob(repo, *id, search_string)
        }
        Change::Deletion { id, .. } => {
            count_matches_in_blob(repo, *id, search_string).map(|count| -count)
        }
        Change::Modification { previous_id, id, .. }
        | Change::Rewrite { source_id: previous_id, id, .. } => {
            let old_count = count_matches_in_blob(repo, *previous_id, search_string)?;
            let new_count = count_matches_in_blob(repo, *id, search_string)?;
            Some(new_count - old_count)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::TestRepo;

    #[test]
    fn test_calculate_commit_delta_addition() {
        let mut repo = TestRepo::new();
        repo.commit("Initial commit", vec![("file.txt", "test test")]);

        let commit_id = repo.last_commit().unwrap();
        let delta = calculate_commit_delta(&repo.thread_safe_repo, &commit_id, "test");
        assert_eq!(delta, 2);
    }

    #[test]
    fn test_calculate_commit_delta_modification() {
        let mut repo = TestRepo::new();
        repo.commit("First commit", vec![("file.txt", "test")]);
        repo.commit("Second commit", vec![("file.txt", "test test test")]);

        let commit_id = repo.last_commit().unwrap();
        let delta = calculate_commit_delta(&repo.thread_safe_repo, &commit_id, "test");
        assert_eq!(delta, 2);
    }

    #[test]
    fn test_calculate_commit_delta_deletion() {
        let mut repo = TestRepo::new();
        repo.commit("First commit", vec![("file.txt", "test test test")]);
        repo.commit("Second commit", vec![("file.txt", "test")]);

        let commit_id = repo.last_commit().unwrap();
        let delta = calculate_commit_delta(&repo.thread_safe_repo, &commit_id, "test");
        assert_eq!(delta, -2);
    }

    #[test]
    fn test_calculate_commit_delta_multiple_files() {
        let mut repo = TestRepo::new();
        repo.commit("First commit", vec![
            ("file1.txt", "test"),
            ("file2.txt", "test test"),
        ]);
        repo.commit("Second commit", vec![
            ("file1.txt", "test test test"),
            ("file2.txt", "test"),
        ]);

        let commit_id = repo.last_commit().unwrap();
        let delta = calculate_commit_delta(&repo.thread_safe_repo, &commit_id, "test");
        assert_eq!(delta, 1);
    }

    #[test]
    fn test_calculate_commit_delta_no_matches() {
        let mut repo = TestRepo::new();
        repo.commit("Initial commit", vec![("file.txt", "hello world")]);

        let commit_id = repo.last_commit().unwrap();
        let delta = calculate_commit_delta(&repo.thread_safe_repo, &commit_id, "test");
        assert_eq!(delta, 0);
    }

    #[test]
    fn test_calculate_commit_delta_case_sensitive() {
        let mut repo = TestRepo::new();
        repo.commit("Initial commit", vec![("file.txt", "Test test TEST")]);

        let commit_id = repo.last_commit().unwrap();
        let delta = calculate_commit_delta(&repo.thread_safe_repo, &commit_id, "test");
        assert_eq!(delta, 1);
    }

    #[test]
    fn test_calculate_commit_delta_mixed_operations() {
        let mut repo = TestRepo::new();
        repo.commit("First commit", vec![
            ("a.txt", "test test"),
            ("b.txt", "test"),
            ("c.txt", "hello world"),
        ]);
        repo.commit("Second commit", vec![
            ("a.txt", "test"),
            ("b.txt", "test test test test"),
            ("c.txt", "test world"),
        ]);

        let commit_id = repo.last_commit().unwrap();
        let delta = calculate_commit_delta(&repo.thread_safe_repo, &commit_id, "test");
        assert_eq!(delta, 3);
    }

    #[test]
    fn test_calculate_commit_delta_file_removal_and_addition() {
        let mut repo = TestRepo::new();
        repo.commit("First commit", vec![
            ("old.txt", "test test test"),
            ("keep.txt", "test"),
        ]);
        repo.commit("Second commit", vec![
            ("keep.txt", "test test"),
            ("new.txt", "test test test test"),
        ]);

        let commit_id = repo.last_commit().unwrap();
        let delta = calculate_commit_delta(&repo.thread_safe_repo, &commit_id, "test");
        assert_eq!(delta, 2);
    }
}

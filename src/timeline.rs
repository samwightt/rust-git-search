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
    use super::*;
    use gix::objs::{Object, Blob, Tree, Commit};
    use gix::prelude::Write;
    use std::sync::OnceLock;

    static TEST_REPO_DIR: OnceLock<tempfile::TempDir> = OnceLock::new();

    /// Get or create the test repository (created once for all tests)
    fn get_base_repo_path() -> &'static std::path::Path {
        let temp_dir = TEST_REPO_DIR.get_or_init(|| {
            let dir = tempfile::tempdir().unwrap();
            gix::init(dir.path()).unwrap();
            dir
        });
        temp_dir.path()
    }

    /// Helper to get an in-memory repository for testing
    fn get_test_repo() -> (ThreadSafeRepository, gix::OdbHandle) {
        let repo_path = get_base_repo_path();
        let repo = gix::open(repo_path).unwrap().with_object_memory();
        let thread_safe = repo.into_sync();
        let objects = thread_safe.to_thread_local().objects.clone();
        (thread_safe, objects)
    }

    /// Helper to create a tree with files
    fn create_tree(odb: &gix::OdbHandle, files: Vec<(&str, &str)>) -> gix::ObjectId {
        let mut entries = Vec::new();

        for (name, content) in files {
            let blob = Blob { data: content.as_bytes().to_vec() };
            let blob_id = odb.write(&Object::Blob(blob)).unwrap();

            entries.push(gix::objs::tree::Entry {
                mode: gix::objs::tree::EntryKind::Blob.into(),
                filename: name.into(),
                oid: blob_id,
            });
        }

        let tree = Tree { entries };
        odb.write(&Object::Tree(tree)).unwrap()
    }

    /// Helper to create a commit
    fn create_commit(
        odb: &gix::OdbHandle,
        tree_id: gix::ObjectId,
        parent_id: Option<gix::ObjectId>,
        message: &str,
    ) -> gix::ObjectId {
        let signature = gix::actor::Signature {
            name: "Test User".into(),
            email: "test@example.com".into(),
            time: gix::date::Time::new(1234567890, 0),
        };

        let parents = match parent_id {
            Some(p) => vec![p].into(),
            None => Default::default(),
        };

        let commit = Commit {
            tree: tree_id,
            parents,
            author: signature.clone(),
            committer: signature,
            encoding: None,
            message: message.into(),
            extra_headers: vec![],
        };

        odb.write(&Object::Commit(commit)).unwrap()
    }

    #[test]
    fn test_calculate_commit_delta_addition() {
        let (thread_safe_repo, odb) = get_test_repo();

        // Create first commit with a file containing "test" twice
        let tree_id = create_tree(&odb, vec![("file.txt", "test test")]);
        let commit_id = create_commit(&odb, tree_id, None, "Initial commit");

        let delta = calculate_commit_delta(&thread_safe_repo, &commit_id, "test");
        assert_eq!(delta, 2);
    }

    #[test]
    fn test_calculate_commit_delta_modification() {
        let (thread_safe_repo, odb) = get_test_repo();

        // First commit: file with "test" once
        let tree1_id = create_tree(&odb, vec![("file.txt", "test")]);
        let commit1_id = create_commit(&odb, tree1_id, None, "First commit");

        // Second commit: modify file to have "test" three times
        let tree2_id = create_tree(&odb, vec![("file.txt", "test test test")]);
        let commit2_id = create_commit(&odb, tree2_id, Some(commit1_id), "Second commit");

        // Delta should be +2 (from 1 to 3)
        let delta = calculate_commit_delta(&thread_safe_repo, &commit2_id, "test");
        assert_eq!(delta, 2);
    }

    #[test]
    fn test_calculate_commit_delta_deletion() {
        let (thread_safe_repo, odb) = get_test_repo();

        // First commit: file with "test" three times
        let tree1_id = create_tree(&odb, vec![("file.txt", "test test test")]);
        let commit1_id = create_commit(&odb, tree1_id, None, "First commit");

        // Second commit: modify file to have "test" once
        let tree2_id = create_tree(&odb, vec![("file.txt", "test")]);
        let commit2_id = create_commit(&odb, tree2_id, Some(commit1_id), "Second commit");

        // Delta should be -2 (from 3 to 1)
        let delta = calculate_commit_delta(&thread_safe_repo, &commit2_id, "test");
        assert_eq!(delta, -2);
    }

    #[test]
    fn test_calculate_commit_delta_multiple_files() {
        let (thread_safe_repo, odb) = get_test_repo();

        // First commit: two files
        let tree1_id = create_tree(&odb, vec![
            ("file1.txt", "test"),
            ("file2.txt", "test test"),
        ]);
        let commit1_id = create_commit(&odb, tree1_id, None, "First commit");

        // Second commit: modify both files
        let tree2_id = create_tree(&odb, vec![
            ("file1.txt", "test test test"), // +2
            ("file2.txt", "test"),             // -1
        ]);
        let commit2_id = create_commit(&odb, tree2_id, Some(commit1_id), "Second commit");

        // Total delta should be +1 (+2 - 1)
        let delta = calculate_commit_delta(&thread_safe_repo, &commit2_id, "test");
        assert_eq!(delta, 1);
    }

    #[test]
    fn test_calculate_commit_delta_no_matches() {
        let (thread_safe_repo, odb) = get_test_repo();

        // Create commit with no matches
        let tree_id = create_tree(&odb, vec![("file.txt", "hello world")]);
        let commit_id = create_commit(&odb, tree_id, None, "Initial commit");

        let delta = calculate_commit_delta(&thread_safe_repo, &commit_id, "test");
        assert_eq!(delta, 0);
    }

    #[test]
    fn test_calculate_commit_delta_case_sensitive() {
        let (thread_safe_repo, odb) = get_test_repo();

        // Create commit with mixed case
        let tree_id = create_tree(&odb, vec![("file.txt", "Test test TEST")]);
        let commit_id = create_commit(&odb, tree_id, None, "Initial commit");

        let delta = calculate_commit_delta(&thread_safe_repo, &commit_id, "test");
        assert_eq!(delta, 1); // Only lowercase "test" matches
    }
}

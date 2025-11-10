use gix::{
    ObjectId, Repository, ThreadSafeRepository, diff::tree_with_rewrites::Change,
};
use rayon::prelude::*;
use serde::Serialize;
use std::{cell::OnceCell, collections::HashSet, fs::File, io::Write, path::PathBuf};

use anyhow::Result;

pub fn timeline(path: &PathBuf, search_string: &str, output: &str) -> Result<()> {
    let thread_safe_repo = ThreadSafeRepository::open(path)?;
    let repo = thread_safe_repo.to_thread_local();

    let head_commit = repo.head()?.peel_to_commit_in_place()?;

    // Collect all commits (ancestors + HEAD)
    let mut commits_with_dupes: Vec<_> = head_commit
        .ancestors()
        .first_parent_only()
        .all()?
        .filter_map(|x| x.ok())
        .map(|x| x.id().detach())
        .collect();

    commits_with_dupes.push(head_commit.id().detach());

    // Remove duplicates (gix sometimes returns HEAD in ancestors)
    let mut seen = HashSet::new();
    let commits: Vec<_> = commits_with_dupes
        .into_iter()
        .filter(|id| seen.insert(*id))
        .collect();

    // Process commits in parallel to calculate deltas
    // Order doesn't matter here - we'll sort by timestamp after
    let mut commit_deltas: Vec<CommitData> = commits
        .par_iter()
        .map(|commit_id| calculate_commit_delta(&thread_safe_repo, commit_id, search_string))
        .collect();

    // Sort by timestamp (oldest first) AFTER parallel processing
    commit_deltas.sort_by_key(|data| data.timestamp);

    // Accumulate running totals sequentially
    let mut running_total: i64 = 0;
    let mut output_file = File::create(output)?;

    for commit_data in commit_deltas {
        running_total += commit_data.delta;

        let entry = TimelineEntry {
            date: commit_data.date,
            commit_id: commit_data.commit_id,
            message: commit_data.message,
            author_name: commit_data.author_name,
            author_email: commit_data.author_email,
            count: running_total,
        };

        writeln!(output_file, "{}", serde_json::to_string(&entry)?)?;
    }

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

struct CommitChange {
    change: Change,
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
) -> CommitData {
    with_repo_cache(thread_safe_repo, |repo| {
        let commit = repo
            .find_commit(*commit_id)
            .expect("Expected git commit to exist");

        // Extract metadata
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

        // Get parent tree
        let parent_tree = commit
            .parent_ids()
            .next()
            .and_then(|x| x.object().ok())
            .and_then(|x| x.peel_to_commit().ok())
            .and_then(|x| x.tree().ok());

        // Calculate delta from changes
        let changes: Vec<CommitChange> = repo
            .diff_tree_to_tree(parent_tree.as_ref(), commit.tree().ok().as_ref(), None)
            .unwrap()
            .into_iter()
            .map(|change| CommitChange { change })
            .collect();

        let delta: i64 = changes
            .iter()
            .filter(|change| {
                change.change.entry_mode().is_blob() && !change.change.entry_mode().is_executable()
            })
            .filter_map(|change| calculate_change_delta(repo, change, search_string))
            .sum();

        CommitData {
            timestamp,
            date,
            commit_id: commit_id.to_string(),
            message,
            author_name,
            author_email,
            delta,
        }
    })
}

fn calculate_change_delta(repo: &Repository, change: &CommitChange, search_string: &str) -> Option<i64> {
    use gix::diff::tree_with_rewrites::Change as DiffChange;

    match &change.change {
        // Addition: new file added
        DiffChange::Addition { id, .. } => {
            let blob = repo.find_blob(*id).ok()?;
            let value = std::str::from_utf8(&blob.data).ok()?;
            let count = value.matches(search_string).count() as i64;
            Some(count)
        }
        // Deletion: file deleted
        DiffChange::Deletion { id, .. } => {
            let blob = repo.find_blob(*id).ok()?;
            let value = std::str::from_utf8(&blob.data).ok()?;
            let count = value.matches(search_string).count() as i64;
            Some(-count) // Negative delta for deletion
        }
        // Modification: file changed
        DiffChange::Modification { previous_id, id, .. } => {
            // Count in old version
            let old_blob = repo.find_blob(*previous_id).ok()?;
            let old_value = std::str::from_utf8(&old_blob.data).ok()?;
            let old_count = old_value.matches(search_string).count() as i64;

            // Count in new version
            let new_blob = repo.find_blob(*id).ok()?;
            let new_value = std::str::from_utf8(&new_blob.data).ok()?;
            let new_count = new_value.matches(search_string).count() as i64;

            Some(new_count - old_count) // Delta is the difference
        }
        // Rewrite: treat as modification
        DiffChange::Rewrite { source_id, id, .. } => {
            // Count in old version
            let old_blob = repo.find_blob(*source_id).ok()?;
            let old_value = std::str::from_utf8(&old_blob.data).ok()?;
            let old_count = old_value.matches(search_string).count() as i64;

            // Count in new version
            let new_blob = repo.find_blob(*id).ok()?;
            let new_value = std::str::from_utf8(&new_blob.data).ok()?;
            let new_count = new_value.matches(search_string).count() as i64;

            Some(new_count - old_count)
        }
    }
}

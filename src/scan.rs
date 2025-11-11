use gix::{
    ObjectId, Repository, ThreadSafeRepository, diff::tree_with_rewrites::Change,
    revision::walk::Sorting, traverse::commit::simple::CommitTimeOrder,
};
use num_format::{Locale, ToFormattedString};
use rayon::prelude::*;
use std::{cell::OnceCell, path::Path};

use anyhow::Result;

pub fn scan(path: &Path, search_string: &str) -> Result<()> {
    let thread_safe_repo = ThreadSafeRepository::open(path)?;
    let repo = thread_safe_repo.to_thread_local();

    let head_commit = repo.head()?.peel_to_commit_in_place()?;
    // Collect all commits (ancestors includes HEAD)
    let items: Vec<_> = head_commit
        .ancestors()
        .sorting(Sorting::ByCommitTime(CommitTimeOrder::OldestFirst))
        .first_parent_only()
        .all()?
        .filter_map(|x| x.ok())
        .map(|x| x.id().detach())
        .collect();

    println!(
        "Got {} commits",
        items.len().to_formatted_string(&Locale::en)
    );

    let res: usize = items
        .par_iter()
        .flat_map(|commit_id| get_commit_changes(&thread_safe_repo, commit_id))
        .filter(|change| {
            change.entry_mode().is_blob() && !change.entry_mode().is_executable()
        })
        .filter_map(|change| process_change(&thread_safe_repo, &change, search_string))
        .sum();

    println!(
        "Number of times '{}' appears in files anywhere in git history: {}",
        search_string,
        res.to_formatted_string(&Locale::en)
    );

    Ok(())
}

thread_local! {
    static REPO_CACHE: OnceCell<gix::Repository> = const { OnceCell::new() };
}

/// Util function that enables creating a single [`Repository`] struct per thread with its own object cache.
/// This saves a ton of time on system calls and enables greater parallelization.
///
/// We use [`ThreadSafeRepository`] in order to process commits and blobs in parallel. But it does not share an object cache
/// between threads. The `ThreadSafeRepository::to_thread_local` function creates a new `Repository` with an object cache,
/// but that object cache is only valid until the Repository gets dropped. So if we create a new `Repository` inside of each `Rayon`
/// task, we're not getting any of the benefits of an object cache.
///
/// To fix that, we use a thread local var that only creates the `Repository` once per _thread_. This means that Rayon tasks on the same
/// thread share the same `Repository` and get the benefits of the object cache. This saves quite a bit of syscalls and speeds things up by
/// about 10-15%.
fn with_repo_cache<R, F: FnOnce(&Repository) -> R>(
    thread_safe_repo: &ThreadSafeRepository,
    f: F,
) -> R {
    REPO_CACHE.with(|cache| {
        let repo = cache.get_or_init(|| thread_safe_repo.to_thread_local());
        f(repo)
    })
}

fn get_commit_changes(
    thread_safe_repo: &ThreadSafeRepository,
    commit_object_id: &ObjectId,
) -> Vec<Change> {
    with_repo_cache(thread_safe_repo, |repo| {
        let commit = repo
            .find_commit(*commit_object_id)
            .expect("Expected git commit to exist (it def should) but for some reason it didn't.");
        let parent_tree = commit
            .parent_ids()
            .next()
            .and_then(|x| x.object().ok())
            .and_then(|x| x.peel_to_commit().ok())
            .and_then(|x| x.tree().ok());

        repo.diff_tree_to_tree(parent_tree.as_ref(), commit.tree().ok().as_ref(), None)
            .unwrap()
            .into_iter()
            .collect()
    })
}

fn process_change(thread_safe_repo: &ThreadSafeRepository, change: &Change, search_string: &str) -> Option<usize> {
    with_repo_cache(thread_safe_repo, |repo| {
        let (_, id) = change.entry_mode_and_id();
        let blob = repo.find_blob(id).unwrap();
        let value = std::str::from_utf8(&blob.data).ok()?;
        Some(value.matches(search_string).count())
    })
}

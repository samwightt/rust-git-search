use gix::{
    ObjectId, Repository, ThreadSafeRepository, diff::tree_with_rewrites::Change,
    revision::walk::Sorting, traverse::commit::simple::CommitTimeOrder,
};
use num_format::{Locale, ToFormattedString};
use rayon::prelude::*;
use std::{cell::OnceCell, path::PathBuf};

use anyhow::Result;

thread_local! {
    static REPO_CACHE: OnceCell<gix::Repository> = const { OnceCell::new() };
}

pub fn scan(path: &PathBuf) -> Result<()> {
    let thread_safe_repo = ThreadSafeRepository::open(path)?;
    let repo = thread_safe_repo.to_thread_local();

    let head_commit = repo.head()?.peel_to_commit_in_place()?;
    let items: Vec<_> = head_commit
        .ancestors()
        .sorting(Sorting::ByCommitTime(CommitTimeOrder::OldestFirst))
        .first_parent_only()
        .all()?
        .filter_map(|x| x.ok())
        .map(|x| x.id().detach())
        .chain(std::iter::once(head_commit.id().detach()))
        .collect();

    println!(
        "Got {} commits",
        items.len().to_formatted_string(&Locale::en)
    );

    let res: usize = items
        .par_iter()
        .flat_map(|commit_id| get_commit_changes(&thread_safe_repo, commit_id).unwrap())
        .filter(|change| {
            change.change.entry_mode().is_blob() && !change.change.entry_mode().is_executable()
        })
        .map(|change| process_change(&thread_safe_repo, &change))
        .sum();

    println!(
        "Total object size: {}",
        res.to_formatted_string(&Locale::en)
    );

    Ok(())
}

struct CommitChange {
    commit_id: ObjectId,
    change: Change,
}

fn process_change(thread_safe_repo: &ThreadSafeRepository, change: &CommitChange) -> usize {
    with_repo_cache(thread_safe_repo, |repo| {
        let (_, id) = change.change.entry_mode_and_id();
        let whatever = repo.find_blob(id).unwrap();
        whatever.data.len()
    })
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

fn get_commit_changes(
    thread_safe_repo: &ThreadSafeRepository,
    commit_object_id: &ObjectId,
) -> Result<Vec<CommitChange>> {
    with_repo_cache(thread_safe_repo, |repo| {
        let commit = repo.find_commit(*commit_object_id)?;
        let parent_tree = commit
            .parent_ids()
            .next()
            .and_then(|x| x.object().ok())
            .and_then(|x| x.peel_to_commit().ok())
            .and_then(|x| x.tree().ok());

        let result: Vec<CommitChange> = repo
            .diff_tree_to_tree(parent_tree.as_ref(), commit.tree().ok().as_ref(), None)?
            .into_iter()
            .map(|change| CommitChange {
                commit_id: *commit_object_id,
                change,
            })
            .collect();

        Ok(result)
    })
}

use gix::{
    ObjectId, ThreadSafeRepository, diff::tree_with_rewrites::Change, revision::walk::Sorting,
    traverse::commit::simple::CommitTimeOrder,
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
        .enumerate()
        .map(|(_index, commit_id)| {
            // Create a thread-local object cache to minimize the amount of fs calls we do.
            let count: Result<usize> = REPO_CACHE.with(|cache| {
                let repo = cache.get_or_init(|| thread_safe_repo.to_thread_local());

                let commit = repo.find_commit(*commit_id)?;
                let parent_tree = commit
                    .parent_ids()
                    .next()
                    .and_then(|x| x.object().ok())
                    .and_then(|x| x.peel_to_commit().ok())
                    .and_then(|x| x.tree().ok());

                let result = repo.diff_tree_to_tree(
                    parent_tree.as_ref(),
                    commit.tree().ok().as_ref(),
                    None,
                )?;

                Ok(result.len())
            });

            count.unwrap()
        })
        .sum();
    println!("Total changes: {}", res.to_formatted_string(&Locale::en));

    Ok(())
}

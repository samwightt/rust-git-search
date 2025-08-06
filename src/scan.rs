use gix::{
    ObjectId, ThreadSafeRepository, revision::walk::Sorting,
    traverse::commit::simple::CommitTimeOrder,
};
use rayon::prelude::*;
use std::path::PathBuf;

use anyhow::Result;

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

    let res: Vec<_> = items
        .par_iter()
        .enumerate()
        .map(|(index, commit_id)| -> Result<(usize, &ObjectId)> {
            let repo = thread_safe_repo.to_thread_local();

            let commit = repo.find_commit(*commit_id)?;
            let parent = commit
                .ancestors()
                .all()?
                .next()
                .transpose()?
                .map(|x| x.id());

            println!("Commit: {}, parent: {:?}", commit.id(), parent);

            Ok((index, commit_id))
        })
        .collect();
    println!("Last commit time: {}", head_commit.id());

    Ok(())
}

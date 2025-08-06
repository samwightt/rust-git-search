use gix::{
    ThreadSafeRepository, revision::walk::Sorting, traverse::commit::simple::CommitTimeOrder,
};
use std::path::PathBuf;

use anyhow::Result;

pub fn scan(path: &PathBuf) -> Result<()> {
    let thread_safe_repo = ThreadSafeRepository::open(path)?;
    let repo = thread_safe_repo.to_thread_local();

    let head_commit = repo.head()?.peel_to_commit_in_place()?;
    let items = head_commit
        .ancestors()
        .sorting(Sorting::ByCommitTime(CommitTimeOrder::OldestFirst))
        .all()?
        .filter_map(|x| x.ok())
        .map(|x| x.id().detach())
        .chain(std::iter::once(head_commit.id().detach()))
        .enumerate();

    for (index, item) in items {
        println!("{index}: {item}");
    }

    println!("Last commit time: {}", head_commit.id());

    Ok(())
}

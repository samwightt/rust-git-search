use std::path::PathBuf;

use anyhow::Result;

pub fn scan(path: &PathBuf) -> Result<()> {
    let repo = gix::open(path)?;

    let head_commit = repo.head()?.peel_to_commit_in_place()?;
    let items = head_commit.ancestors().all()?.filter_map(|x| x.ok());

    for item in items {
        println!("Commit: {}", item.id());
    }

    Ok(())
}

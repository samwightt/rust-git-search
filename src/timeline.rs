use gix::{
    ObjectId, Repository, ThreadSafeRepository, diff::tree_with_rewrites::Change,
};
use lasso::{Spur, ThreadedRodeo};
use rayon::prelude::*;
use regex::Regex;
use serde::Serialize;
use std::{cell::OnceCell, collections::HashMap, fs::File, io::Write, path::Path};

use anyhow::Result;

pub enum SearchOptions {
    Literal { pattern: String },
    CaseInsensitive { lower_pattern: String },
    Regex { regex: Regex },
}

impl SearchOptions {
    pub fn new(pattern: &str, case_insensitive: bool, use_regex: bool) -> Result<Self> {
        match (use_regex, case_insensitive) {
            (true, _) => Self::regex(pattern), // case_insensitive ignored for regex
            (false, true) => Ok(Self::case_insensitive(pattern)),
            (false, false) => Ok(Self::literal(pattern)),
        }
    }

    pub fn literal(pattern: &str) -> Self {
        SearchOptions::Literal {
            pattern: pattern.to_string(),
        }
    }

    pub fn case_insensitive(pattern: &str) -> Self {
        SearchOptions::CaseInsensitive {
            lower_pattern: pattern.to_lowercase(),
        }
    }

    pub fn regex(pattern: &str) -> Result<Self> {
        Ok(SearchOptions::Regex {
            regex: Regex::new(pattern)?,
        })
    }

    fn count_matches(&self, text: &str) -> i64 {
        match self {
            SearchOptions::Regex { regex } => regex.find_iter(text).count() as i64,
            SearchOptions::CaseInsensitive { lower_pattern } => {
                let lower_text = text.to_lowercase();
                lower_text.matches(lower_pattern.as_str()).count() as i64
            }
            SearchOptions::Literal { pattern } => text.matches(pattern.as_str()).count() as i64,
        }
    }
}

pub fn timeline(path: &Path, search_string: &str, output: &str, case_insensitive: bool, use_regex: bool, use_codeowners: bool) -> Result<()> {
    let search_options = SearchOptions::new(search_string, case_insensitive, use_regex)?;
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

    // Create string interner for codeowners if needed
    let interner = if use_codeowners {
        Some(ThreadedRodeo::new())
    } else {
        None
    };

    // Process commits in parallel: calculate deltas, then extract metadata
    // Order doesn't matter here - we'll sort by timestamp after
    let mut commit_deltas: Vec<CommitData> = commits
        .par_iter()
        .map(|commit_id| {
            calculate_commit_delta(&thread_safe_repo, commit_id, &search_options, interner.as_ref())
        })
        .map(|result| extract_commit_metadata(&thread_safe_repo, result))
        .collect();

    // Convert to resolver (read-only) after all writes are done
    let resolver = interner.map(|rodeo| rodeo.into_resolver());

    // Sort by timestamp (oldest first) AFTER parallel processing
    commit_deltas.par_sort_by_key(|data| data.timestamp);

    // Accumulate running totals sequentially and write output
    let mut output_file = File::create(output)?;
    let mut running_total = 0i64;
    let mut owner_running_totals: HashMap<Spur, i64> = HashMap::new();

    for commit_data in commit_deltas {
        running_total += commit_data.delta;

        let codeowners = if let Some(ref resolver) = resolver {
            if let Some(ref owner_deltas) = commit_data.owner_deltas {
                for (owner_key, delta) in owner_deltas {
                    *owner_running_totals.entry(*owner_key).or_insert(0) += delta;
                }
            }
            // Resolve Spur keys back to strings for JSON output
            Some(
                owner_running_totals
                    .iter()
                    .map(|(key, value)| (resolver.resolve(key).to_string(), *value))
                    .collect()
            )
        } else {
            None
        };

        let entry = TimelineEntry {
            date: commit_data.date,
            commit_id: commit_data.commit_id,
            message: commit_data.message,
            author_name: commit_data.author_name,
            author_email: commit_data.author_email,
            count: running_total,
            codeowners,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    codeowners: Option<HashMap<String, i64>>,
}

struct CommitData {
    timestamp: i64,
    date: String,
    commit_id: String,
    message: String,
    author_name: String,
    author_email: String,
    delta: i64,
    owner_deltas: Option<HashMap<Spur, i64>>,
}

struct DeltaResult {
    commit_id: ObjectId,
    delta: i64,
    owner_deltas: Option<HashMap<Spur, i64>>,
}

fn calculate_commit_delta(
    thread_safe_repo: &ThreadSafeRepository,
    commit_id: &ObjectId,
    search_options: &SearchOptions,
    interner: Option<&ThreadedRodeo>,
) -> DeltaResult {
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

        // Parse CODEOWNERS if needed
        let codeowners_file = if interner.is_some() {
            read_codeowners_from_commit(repo, &commit)
        } else {
            None
        };

        let mut total_delta = 0i64;
        let mut owner_deltas: HashMap<Spur, i64> = HashMap::new();

        // Calculate delta from changes
        repo
            .diff_tree_to_tree(parent_tree.as_ref(), commit.tree().ok().as_ref(), None)
            .unwrap()
            .into_iter()
            .filter(|change| {
                change.entry_mode().is_blob() && !change.entry_mode().is_executable()
            })
            .for_each(|change| {
                if let Some(delta) = calculate_change_delta(repo, &change, search_options) {
                    total_delta += delta;

                    if let Some(interner) = interner
                        && let Some(path) = get_change_path(&change) {
                            let owners = determine_owners(&codeowners_file, &path, interner);
                            for owner_key in owners {
                                *owner_deltas.entry(owner_key).or_insert(0) += delta;
                            }
                        }
                }
            });

        DeltaResult {
            commit_id: *commit_id,
            delta: total_delta,
            owner_deltas: if interner.is_some() { Some(owner_deltas) } else { None },
        }
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
            owner_deltas: result.owner_deltas,
        }
    })
}

fn calculate_change_delta(repo: &Repository, change: &Change, search_options: &SearchOptions) -> Option<i64> {
    match change {
        Change::Addition { id, .. } => {
            count_matches_in_blob(repo, *id, search_options)
        }
        Change::Deletion { id, .. } => {
            count_matches_in_blob(repo, *id, search_options).map(|count| -count)
        }
        Change::Modification { previous_id, id, .. }
        | Change::Rewrite { source_id: previous_id, id, .. } => {
            let old_count = count_matches_in_blob(repo, *previous_id, search_options)?;
            let new_count = count_matches_in_blob(repo, *id, search_options)?;
            Some(new_count - old_count)
        }
    }
}

fn count_matches_in_blob(repo: &Repository, id: ObjectId, search_options: &SearchOptions) -> Option<i64> {
    repo.find_blob(id).ok().and_then(|blob| {
        std::str::from_utf8(&blob.data)
            .ok()
            .map(|text| search_options.count_matches(text))
    })
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

fn read_codeowners_from_commit(_repo: &Repository, commit: &gix::Commit) -> Option<codeowners::Owners> {
    let tree = commit.tree().ok()?;

    // Try common CODEOWNERS locations in GitHub priority order
    let possible_paths = ["CODEOWNERS", ".github/CODEOWNERS", "docs/CODEOWNERS"];

    for path in possible_paths {
        if let Some(entry) = tree.lookup_entry_by_path(path).ok().flatten()
            && let Ok(blob) = entry.object().ok()?.try_into_blob()
            && let Ok(content) = std::str::from_utf8(&blob.data) {
                let owners = codeowners::from_reader(content.as_bytes());
                return Some(owners);
            }
    }

    None
}

fn get_change_path(change: &Change) -> Option<String> {
    match change {
        Change::Addition { location, .. }
        | Change::Deletion { location, .. }
        | Change::Modification { location, .. }
        | Change::Rewrite { location, .. } => {
            Some(location.to_string())
        }
    }
}

fn determine_owners(codeowners_file: &Option<codeowners::Owners>, path: &str, interner: &ThreadedRodeo) -> Vec<Spur> {
    if let Some(owners_file) = codeowners_file
        && let Some(owners) = owners_file.of(path) {
            // Intern each owner separately and return all keys
            return owners.iter()
                .map(|o| interner.get_or_intern(o.to_string()))
                .collect();
        }

    // Default to "unowned" if no owner found
    vec![interner.get_or_intern("unowned")]
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
        let search_options = SearchOptions::literal("test");
        let result = calculate_commit_delta(&repo.thread_safe_repo, &commit_id, &search_options, None);
        assert_eq!(result.delta, 2);
    }

    #[test]
    fn test_calculate_commit_delta_modification() {
        let mut repo = TestRepo::new();
        repo.commit("First commit", vec![("file.txt", "test")]);
        repo.commit("Second commit", vec![("file.txt", "test test test")]);

        let commit_id = repo.last_commit().unwrap();
        let search_options = SearchOptions::literal("test");
        let result = calculate_commit_delta(&repo.thread_safe_repo, &commit_id, &search_options, None);
        assert_eq!(result.delta, 2);
    }

    #[test]
    fn test_calculate_commit_delta_deletion() {
        let mut repo = TestRepo::new();
        repo.commit("First commit", vec![("file.txt", "test test test")]);
        repo.commit("Second commit", vec![("file.txt", "test")]);

        let commit_id = repo.last_commit().unwrap();
        let search_options = SearchOptions::literal("test");
        let result = calculate_commit_delta(&repo.thread_safe_repo, &commit_id, &search_options, None);
        assert_eq!(result.delta, -2);
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
        let search_options = SearchOptions::literal("test");
        let result = calculate_commit_delta(&repo.thread_safe_repo, &commit_id, &search_options, None);
        assert_eq!(result.delta, 1);
    }

    #[test]
    fn test_calculate_commit_delta_no_matches() {
        let mut repo = TestRepo::new();
        repo.commit("Initial commit", vec![("file.txt", "hello world")]);

        let commit_id = repo.last_commit().unwrap();
        let search_options = SearchOptions::literal("test");
        let result = calculate_commit_delta(&repo.thread_safe_repo, &commit_id, &search_options, None);
        assert_eq!(result.delta, 0);
    }

    #[test]
    fn test_calculate_commit_delta_case_sensitive() {
        let mut repo = TestRepo::new();
        repo.commit("Initial commit", vec![("file.txt", "Test test TEST")]);

        let commit_id = repo.last_commit().unwrap();
        let search_options = SearchOptions::literal("test");
        let result = calculate_commit_delta(&repo.thread_safe_repo, &commit_id, &search_options, None);
        assert_eq!(result.delta, 1);
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
        let search_options = SearchOptions::literal("test");
        let result = calculate_commit_delta(&repo.thread_safe_repo, &commit_id, &search_options, None);
        assert_eq!(result.delta, 3);
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
        let search_options = SearchOptions::literal("test");
        let result = calculate_commit_delta(&repo.thread_safe_repo, &commit_id, &search_options, None);
        assert_eq!(result.delta, 2);
    }

    #[test]
    fn test_case_insensitive_search() {
        let mut repo = TestRepo::new();
        repo.commit("Initial commit", vec![("file.txt", "Test test TEST")]);

        let commit_id = repo.last_commit().unwrap();
        let search_options = SearchOptions::case_insensitive("test");
        let result = calculate_commit_delta(&repo.thread_safe_repo, &commit_id, &search_options, None);
        assert_eq!(result.delta, 3);
    }

    #[test]
    fn test_case_insensitive_modification() {
        let mut repo = TestRepo::new();
        repo.commit("First commit", vec![("file.txt", "TEST")]);
        repo.commit("Second commit", vec![("file.txt", "Test test TEST")]);

        let commit_id = repo.last_commit().unwrap();
        let search_options = SearchOptions::case_insensitive("test");
        let result = calculate_commit_delta(&repo.thread_safe_repo, &commit_id, &search_options, None);
        assert_eq!(result.delta, 2);
    }

    #[test]
    fn test_regex_search_simple() {
        let mut repo = TestRepo::new();
        repo.commit("Initial commit", vec![("file.txt", "test Test testing")]);

        let commit_id = repo.last_commit().unwrap();
        let search_options = SearchOptions::regex(r"test").unwrap();
        let result = calculate_commit_delta(&repo.thread_safe_repo, &commit_id, &search_options, None);
        assert_eq!(result.delta, 2); // matches "test" and "testing"
    }

    #[test]
    fn test_regex_case_insensitive() {
        let mut repo = TestRepo::new();
        repo.commit("Initial commit", vec![("file.txt", "Test test TEST testing")]);

        let commit_id = repo.last_commit().unwrap();
        let search_options = SearchOptions::regex(r"(?i)test").unwrap();
        let result = calculate_commit_delta(&repo.thread_safe_repo, &commit_id, &search_options, None);
        assert_eq!(result.delta, 4); // matches "Test", "test", "TEST", "testing"
    }

    #[test]
    fn test_regex_pattern() {
        let mut repo = TestRepo::new();
        repo.commit("Initial commit", vec![("file.txt", "test123 test456 testing")]);

        let commit_id = repo.last_commit().unwrap();
        let search_options = SearchOptions::regex(r"test\d+").unwrap();
        let result = calculate_commit_delta(&repo.thread_safe_repo, &commit_id, &search_options, None);
        assert_eq!(result.delta, 2); // matches "test123" and "test456"
    }

    #[test]
    fn test_codeowners_basic() {
        let mut repo = TestRepo::new();
        repo.commit("Add CODEOWNERS", vec![
            ("CODEOWNERS", "*.rs @rust-team\n*.md @docs-team\n"),
            ("file.rs", "test"),
        ]);

        let commit_id = repo.last_commit().unwrap();
        let search_options = SearchOptions::literal("test");
        let interner = ThreadedRodeo::new();
        let result = calculate_commit_delta(&repo.thread_safe_repo, &commit_id, &search_options, Some(&interner));

        assert_eq!(result.delta, 1);
        assert!(result.owner_deltas.is_some());

        let owner_deltas = result.owner_deltas.unwrap();
        let rust_team_key = interner.get("@rust-team").unwrap();
        assert_eq!(owner_deltas.get(&rust_team_key), Some(&1));
    }

    #[test]
    fn test_codeowners_multiple_files() {
        let mut repo = TestRepo::new();
        repo.commit("Add files", vec![
            ("CODEOWNERS", "*.rs @rust-team\n*.md @docs-team\n"),
            ("code.rs", "test test"),
            ("readme.md", "test"),
        ]);

        let commit_id = repo.last_commit().unwrap();
        let search_options = SearchOptions::literal("test");
        let interner = ThreadedRodeo::new();
        let result = calculate_commit_delta(&repo.thread_safe_repo, &commit_id, &search_options, Some(&interner));

        assert_eq!(result.delta, 3);
        assert!(result.owner_deltas.is_some());

        let owner_deltas = result.owner_deltas.unwrap();
        let rust_team_key = interner.get("@rust-team").unwrap();
        let docs_team_key = interner.get("@docs-team").unwrap();
        assert_eq!(owner_deltas.get(&rust_team_key), Some(&2));
        assert_eq!(owner_deltas.get(&docs_team_key), Some(&1));
    }

    #[test]
    fn test_codeowners_unowned_files() {
        let mut repo = TestRepo::new();
        repo.commit("Add files", vec![
            ("CODEOWNERS", "*.rs @rust-team\n"),
            ("code.rs", "test"),
            ("other.txt", "test test"),
        ]);

        let commit_id = repo.last_commit().unwrap();
        let search_options = SearchOptions::literal("test");
        let interner = ThreadedRodeo::new();
        let result = calculate_commit_delta(&repo.thread_safe_repo, &commit_id, &search_options, Some(&interner));

        assert_eq!(result.delta, 3);
        assert!(result.owner_deltas.is_some());

        let owner_deltas = result.owner_deltas.unwrap();
        let rust_team_key = interner.get("@rust-team").unwrap();
        let unowned_key = interner.get("unowned").unwrap();
        assert_eq!(owner_deltas.get(&rust_team_key), Some(&1));
        assert_eq!(owner_deltas.get(&unowned_key), Some(&2));
    }

    #[test]
    fn test_codeowners_modification() {
        let mut repo = TestRepo::new();
        repo.commit("First commit", vec![
            ("CODEOWNERS", "*.rs @rust-team\n"),
            ("code.rs", "test"),
        ]);
        repo.commit("Second commit", vec![
            ("CODEOWNERS", "*.rs @rust-team\n"),
            ("code.rs", "test test test"),
        ]);

        let commit_id = repo.last_commit().unwrap();
        let search_options = SearchOptions::literal("test");
        let interner = ThreadedRodeo::new();
        let result = calculate_commit_delta(&repo.thread_safe_repo, &commit_id, &search_options, Some(&interner));

        assert_eq!(result.delta, 2);
        assert!(result.owner_deltas.is_some());

        let owner_deltas = result.owner_deltas.unwrap();
        let rust_team_key = interner.get("@rust-team").unwrap();
        assert_eq!(owner_deltas.get(&rust_team_key), Some(&2));
    }

    #[test]
    fn test_codeowners_disabled() {
        let mut repo = TestRepo::new();
        repo.commit("Add files", vec![
            ("CODEOWNERS", "*.rs @rust-team\n"),
            ("code.rs", "test"),
        ]);

        let commit_id = repo.last_commit().unwrap();
        let search_options = SearchOptions::literal("test");
        let result = calculate_commit_delta(&repo.thread_safe_repo, &commit_id, &search_options, None);

        assert_eq!(result.delta, 1);
        assert!(result.owner_deltas.is_none());
    }

    #[test]
    fn test_codeowners_multiple_owners_single_file() {
        let mut repo = TestRepo::new();
        repo.commit("Add files", vec![
            ("CODEOWNERS", "*.rs @team1 @team2\n"),
            ("code.rs", "test test test"),
        ]);

        let commit_id = repo.last_commit().unwrap();
        let search_options = SearchOptions::literal("test");
        let interner = ThreadedRodeo::new();
        let result = calculate_commit_delta(&repo.thread_safe_repo, &commit_id, &search_options, Some(&interner));

        assert_eq!(result.delta, 3);
        assert!(result.owner_deltas.is_some());

        let owner_deltas = result.owner_deltas.unwrap();
        let team1_key = interner.get("@team1").unwrap();
        let team2_key = interner.get("@team2").unwrap();
        // Both teams should get all 3 matches
        assert_eq!(owner_deltas.get(&team1_key), Some(&3));
        assert_eq!(owner_deltas.get(&team2_key), Some(&3));
    }

    #[test]
    fn test_codeowners_ownership_change() {
        let mut repo = TestRepo::new();
        // First commit: owned by team1
        repo.commit("First commit", vec![
            ("CODEOWNERS", "*.rs @team1\n"),
            ("code.rs", "test test"),
        ]);
        // Second commit: ownership changed to team2
        repo.commit("Second commit", vec![
            ("CODEOWNERS", "*.rs @team2\n"),
            ("code.rs", "test test test"),
        ]);

        let commit_id = repo.last_commit().unwrap();
        let search_options = SearchOptions::literal("test");
        let interner = ThreadedRodeo::new();
        let result = calculate_commit_delta(&repo.thread_safe_repo, &commit_id, &search_options, Some(&interner));

        assert_eq!(result.delta, 1);
        assert!(result.owner_deltas.is_some());

        let owner_deltas = result.owner_deltas.unwrap();
        let team2_key = interner.get("@team2").unwrap();
        // Only team2 should get the delta in this commit (per-commit parsing)
        assert_eq!(owner_deltas.get(&team2_key), Some(&1));
        // team1 should not appear
        assert!(interner.get("@team1").is_none() || owner_deltas.get(&interner.get("@team1").unwrap()).is_none());
    }
}

use gix::objs::{Object, Blob, Tree, Commit};
use gix::prelude::Write;
use gix::{ObjectId, ThreadSafeRepository};
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

/// Test repository builder that tracks commit history
pub struct TestRepo {
    pub thread_safe_repo: ThreadSafeRepository,
    odb: gix::OdbHandle,
    last_commit: Option<ObjectId>,
}

impl TestRepo {
    pub fn new() -> Self {
        let repo_path = get_base_repo_path();
        let repo = gix::open(repo_path).unwrap().with_object_memory();
        let thread_safe = repo.into_sync();
        let objects = thread_safe.to_thread_local().objects.clone();

        Self {
            thread_safe_repo: thread_safe,
            odb: objects,
            last_commit: None,
        }
    }

    /// Create a commit with files, automatically using the last commit as parent
    pub fn commit(&mut self, message: &str, files: Vec<(&str, &str)>) -> ObjectId {
        let tree_id = self.create_tree(files);
        let commit_id = self.create_commit_obj(tree_id, self.last_commit, message);
        self.last_commit = Some(commit_id);
        commit_id
    }

    /// Get the most recent commit ID
    pub fn last_commit(&self) -> Option<ObjectId> {
        self.last_commit
    }

    fn create_tree(&self, files: Vec<(&str, &str)>) -> ObjectId {
        let mut entries = Vec::new();

        for (name, content) in files {
            let blob = Blob { data: content.as_bytes().to_vec() };
            let blob_id = self.odb.write(&Object::Blob(blob)).unwrap();

            entries.push(gix::objs::tree::Entry {
                mode: gix::objs::tree::EntryKind::Blob.into(),
                filename: name.into(),
                oid: blob_id,
            });
        }

        entries.sort_by(|a, b| a.filename.cmp(&b.filename));

        let tree = Tree { entries };
        self.odb.write(&Object::Tree(tree)).unwrap()
    }

    fn create_commit_obj(
        &self,
        tree_id: ObjectId,
        parent_id: Option<ObjectId>,
        message: &str,
    ) -> ObjectId {
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

        self.odb.write(&Object::Commit(commit)).unwrap()
    }
}

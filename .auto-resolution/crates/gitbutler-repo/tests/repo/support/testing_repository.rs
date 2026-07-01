use but_testsupport::gix_testtools::{Creation, scripted_fixture_writable_with_args};
use tempfile::TempDir;

pub struct TestingRepository {
    pub fixture_dir: TempDir,
}

impl TestingRepository {
    pub fn from_fixture(fixture: &str) -> Self {
        let tempdir = scripted_fixture_writable_with_args(
            format!("scenario/{fixture}.sh"),
            None::<String>,
            Creation::CopyFromReadOnly,
        )
        .map_err(anyhow::Error::from_boxed)
        .expect("fixture should materialize");
        Self {
            fixture_dir: tempdir,
        }
    }

    pub fn open_with_initial_commit(files: &[(&str, &str)]) -> Self {
        let fixture = match files {
            [] => "create-wd-tree-empty",
            [("file1.txt", "content1")] => "create-wd-tree-file1-content1",
            [("file1.txt", "content1"), ("file2.txt", "content2")] => "create-wd-tree-two-files",
            [
                ("dir1/file1.txt", "content1"),
                ("dir2/file2.txt", "content2"),
            ] => "create-wd-tree-two-directories",
            [("target", "helloworld")] => "create-wd-tree-target-file",
            [("soon-directory", "this tracked file becomes a directory")] => {
                "create-wd-tree-soon-directory"
            }
            [
                ("soon-file/content", "this tracked is removed and the parent dir becomes a file"),
            ] => "create-wd-tree-soon-file",
            [("soon-fifo", "actual content")] => "create-wd-tree-soon-fifo",
            [("tracked", "content"), (".gitignore", "*.ignored")] => "create-wd-tree-ignored-files",
            [("soon-empty", "content")] => "create-wd-tree-soon-empty",
            [("soon-too-big", "still small enough")] => "create-wd-tree-soon-too-big",
            other => panic!("unsupported create_wd_tree fixture: {other:?}"),
        };
        Self::from_fixture(fixture)
    }
}

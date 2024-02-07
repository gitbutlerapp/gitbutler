//! CLI-based (fork/exec) backend implementation,
//! executing the `git` command-line tool available
//! on `$PATH`.

mod executor;
mod repository;

#[cfg(unix)]
pub use self::executor::Uid;
pub use self::{
    executor::{AskpassServer, FileStat, GitExecutor, Pid, Socket},
    repository::Repository,
};

#[cfg(feature = "tokio")]
pub use self::executor::tokio;

#[cfg(test)]
mod tests {
    use super::*;

    async fn make_repo(test_name: String) -> impl crate::Repository {
        let repo_path = std::env::temp_dir()
            .join("gitbutler-tests")
            .join("git")
            .join("cli")
            .join(test_name);
        let _ = std::fs::remove_dir_all(&repo_path);
        std::fs::create_dir_all(&repo_path).unwrap();

        Repository::open_or_init(executor::tokio::TokioExecutor, repo_path.to_str().unwrap())
            .await
            .unwrap()
    }

    crate::gitbutler_git_integration_tests!(make_repo, enable_io);
}

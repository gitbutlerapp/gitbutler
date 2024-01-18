//! CLI-based (fork/exec) backend implementation,
//! executing the `git` command-line tool available
//! on `$PATH`.

mod executor;
mod repository;

pub use self::{executor::GitExecutor, repository::Repository};

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
        Repository::open_or_init(executor::tokio::TokioExecutor, repo_path)
            .await
            .unwrap()
    }

    crate::gitbutler_git_integration_tests!(make_repo);
}

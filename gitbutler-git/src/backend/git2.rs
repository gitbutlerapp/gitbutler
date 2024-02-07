//! [libgit2](https://libgit2.org/) implementation of
//! the core `gitbutler-git` library traits.
//!
//! The entry point for this module is the [`Repository`] struct.

mod repository;
mod thread_resource;

#[cfg(feature = "tokio")]
pub use self::thread_resource::tokio;

pub use self::{
    repository::Repository,
    thread_resource::{ThreadedResource, ThreadedResourceHandle},
};

#[cfg(test)]
mod tests {
    use super::*;

    async fn make_repo(test_name: String) -> impl crate::Repository {
        let repo_path = std::env::temp_dir()
            .join("gitbutler-tests")
            .join("git")
            .join("git2")
            .join(test_name);
        let _ = std::fs::remove_dir_all(&repo_path);
        std::fs::create_dir_all(&repo_path).unwrap();

        Repository::<tokio::TokioThreadedResource>::open_or_init(&repo_path)
            .await
            .unwrap()
    }

    crate::gitbutler_git_integration_tests!(make_repo, disable_io);
}

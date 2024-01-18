/// To use in a backend, create a function that initializes
/// an empty repository, whatever that looks like, and returns
/// something that implements the `Repository` trait.
///
/// Include this file via
/// `include!(concat!(env!("CARGO_MANIFEST_DIR"), "/integration-tests.rs"));`
///
/// Then, pass the function to `gitbutler_git_integration_tests!(fn)`, like so:
///
/// ```
/// #[cfg(test)]
/// mod tests {
///     async fn make_repo(test_name: String) -> impl crate::Repository {
///         // Use `test_name` to create a unique repository, if needed.
///         todo!();
///     }
///
///    crate::gitbutler_git_integration_tests!(make_repo);
/// }
/// ```
macro_rules! gitbutler_git_integration_tests {
    ($create_repo:expr) => {
        $crate::gitbutler_git_integration_tests! {
            $create_repo,

            async fn create_repo_selftest(_repo) {
                // Do-nothing, just a selftest.
            }

            async fn check_utmost_discretion(repo) {
                assert_eq!(crate::ops::has_utmost_discretion(&repo).await.unwrap(), false);
                crate::ops::set_utmost_discretion(&repo, true).await.unwrap();
                assert_eq!(crate::ops::has_utmost_discretion(&repo).await.unwrap(), true);
                crate::ops::set_utmost_discretion(&repo, false).await.unwrap();
                assert_eq!(crate::ops::has_utmost_discretion(&repo).await.unwrap(), false);
            }
        }
    };

    // Don't use this one from your backend. This is an internal macro.
    ($create_repo:expr, $(async fn $name:ident($repo:ident) { $($body:tt)* })*) => {
        $(
            #[test]
            fn $name() {
                ::tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let $repo = $create_repo({
                        let mod_name = ::std::module_path!();
                        let test_name = ::std::stringify!($name);
                        format!("{mod_name}::{test_name}")
                    }).await;

                    $($body)*
                })
            }
        )*
    }
}

pub(crate) use gitbutler_git_integration_tests;

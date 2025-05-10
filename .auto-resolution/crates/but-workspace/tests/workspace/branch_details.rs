/// All tests have a workspace present.
mod with_workspace {
    use crate::utils::read_only_in_memory_scenario;
    use but_core::RefMetadata;
    use but_core::ref_metadata::{Branch, Workspace};
    use but_testsupport::visualize_commit_graph;
    use gix::refs::{FullName, FullNameRef};
    use std::any::Any;
    use std::ops::{Deref, DerefMut};

    fn refname(short_name: &str) -> gix::refs::FullName {
        format!("refs/heads/{short_name}").try_into().unwrap()
    }

    #[test]
    #[ignore = "TBD"]
    fn merge_with_two_branches() -> anyhow::Result<()> {
        let repo = read_only_in_memory_scenario("merge-with-two-branches-line-offset")?;
        insta::assert_snapshot!(visualize_commit_graph(&repo, "HEAD")?, @r"
        *   2a6d103 (HEAD -> merge) Merge branch 'A' into merge
        |\  
        | * 7f389ed (A) add 10 to the beginning
        * | 91ef6f6 (B) add 10 to the end
        |/  
        * ff045ef (main) init
        ");
        let store = WorkspaceStore::with_target("A");
        insta::assert_debug_snapshot!(
            but_workspace::branch_details_v3(&repo, refname("A").as_ref(), &store).unwrap(),
            @r"",
        );
        Ok(())
    }

    struct WorkspaceStore {
        workspace: but_core::ref_metadata::Workspace,
    }

    impl WorkspaceStore {
        pub fn with_target(short_name: &str) -> Self {
            WorkspaceStore {
                workspace: but_core::ref_metadata::Workspace {
                    ref_info: Default::default(),
                    stacks: vec![],
                    target_ref: Some(refname(short_name)),
                },
            }
        }
    }

    struct NullHandle<T> {
        inner: T,
        name: FullName,
    }

    impl<T> but_core::ref_metadata::ValueInfo for NullHandle<T> {
        fn is_default(&self) -> bool {
            false
        }
    }

    impl<T> Deref for NullHandle<T> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            &self.inner
        }
    }

    impl<T> DerefMut for NullHandle<T> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.inner
        }
    }

    impl<T> AsRef<FullNameRef> for NullHandle<T> {
        fn as_ref(&self) -> &FullNameRef {
            self.name.as_ref()
        }
    }

    impl RefMetadata for WorkspaceStore {
        type Handle<T> = NullHandle<T>;

        fn iter(&self) -> impl Iterator<Item = anyhow::Result<(FullName, Box<dyn Any>)>> + '_ {
            std::iter::empty()
        }

        fn workspace(&self, ref_name: &FullNameRef) -> anyhow::Result<Self::Handle<Workspace>> {
            Ok(NullHandle {
                inner: self.workspace.clone(),
                name: ref_name.into(),
            })
        }

        fn branch(&self, _ref_name: &FullNameRef) -> anyhow::Result<Self::Handle<Branch>> {
            unreachable!()
        }

        fn set_workspace(&mut self, _value: &Self::Handle<Workspace>) -> anyhow::Result<()> {
            unreachable!()
        }

        fn set_branch(&mut self, _value: &Self::Handle<Branch>) -> anyhow::Result<()> {
            unreachable!()
        }

        fn remove(&mut self, _ref_name: &FullNameRef) -> anyhow::Result<bool> {
            unreachable!()
        }
    }
}

use crate::head_info::utils::read_only_in_memory_scenario;
use but_workspace::head_info;

#[test]
fn untracked() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("unborn-untracked")?;
    let info = but_workspace::head_info(
        &repo,
        &*meta,
        head_info::Options {
            stack_commit_limit: 5,
        },
    )?;
    insta::assert_debug_snapshot!(&info, @r#"
    HeadInfo {
        stacks: [
            Stack {
                index: 0,
                tip: None,
                segments: [
                    StackSegment {
                        ref_name: Some(
                            FullName(
                                "refs/heads/main",
                            ),
                        ),
                        ref_location: Some(
                            AtHead,
                        ),
                        commits_unique_from_tip: [],
                        commits_unintegratd_local: [],
                        commits_unintegrated_upstream: [],
                        remote_tracking_ref_name: None,
                        metadata: None,
                    },
                ],
                stash_status: None,
            },
        ],
        target_ref: None,
    }
    "#);
    Ok(())
}

#[test]
fn detached() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("one-commit-detached")?;
    let info = but_workspace::head_info(
        &repo,
        &*meta,
        head_info::Options {
            stack_commit_limit: 0,
        },
    )?;
    insta::assert_debug_snapshot!(&info, @r"
    HeadInfo {
        stacks: [],
        target_ref: None,
    }
    ");
    Ok(())
}

#[test]
fn single_branch() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("single-branch-10-commits")?;
    let stack_commit_limit = 5;

    let info = but_workspace::head_info(&repo, &*meta, head_info::Options { stack_commit_limit })?;

    assert_eq!(
        info.stacks[0].segments.len(),
        1,
        "a single branch, a single segment"
    );
    assert_eq!(
        info.stacks[0].segments[0].commits_unique_from_tip.len(),
        stack_commit_limit,
        "commit limit is respected"
    );
    insta::assert_debug_snapshot!(&info, @r#"
    HeadInfo {
        stacks: [
            Stack {
                index: 0,
                tip: Some(
                    Sha1(b5743a3aa79957bcb7f654d7d4ad11d995ad5303),
                ),
                segments: [
                    StackSegment {
                        ref_name: Some(
                            FullName(
                                "refs/heads/main",
                            ),
                        ),
                        ref_location: Some(
                            AtHead,
                        ),
                        commits_unique_from_tip: [
                            BranchCommit {
                                id: Sha1(b5743a3aa79957bcb7f654d7d4ad11d995ad5303),
                                title: "10\n",
                                committed_date: Time {
                                    seconds: 946771200,
                                    offset: 0,
                                    sign: Plus,
                                },
                            },
                            BranchCommit {
                                id: Sha1(344e3209e344c1eb90bedb4b00b4d4999a84406c),
                                title: "9\n",
                                committed_date: Time {
                                    seconds: 946771200,
                                    offset: 0,
                                    sign: Plus,
                                },
                            },
                            BranchCommit {
                                id: Sha1(599c271e8734e58a96b3a22666704c2a72623f7f),
                                title: "8\n",
                                committed_date: Time {
                                    seconds: 946771200,
                                    offset: 0,
                                    sign: Plus,
                                },
                            },
                            BranchCommit {
                                id: Sha1(05f069b1c601c098170571bc9fab6966606f8b51),
                                title: "7\n",
                                committed_date: Time {
                                    seconds: 946771200,
                                    offset: 0,
                                    sign: Plus,
                                },
                            },
                            BranchCommit {
                                id: Sha1(c4f2a356d6ed7250bab3dd7c58e1922b95f288c5),
                                title: "6\n",
                                committed_date: Time {
                                    seconds: 946771200,
                                    offset: 0,
                                    sign: Plus,
                                },
                            },
                        ],
                        commits_unintegratd_local: [],
                        commits_unintegrated_upstream: [],
                        remote_tracking_ref_name: None,
                        metadata: None,
                    },
                ],
                stash_status: None,
            },
        ],
        target_ref: None,
    }
    "#);
    Ok(())
}

#[test]
fn single_branch_multiple_segments() -> anyhow::Result<()> {
    let (repo, meta) = read_only_in_memory_scenario("single-branch-10-commits-multi-segment")?;
    let info = but_workspace::head_info(
        &repo,
        &*meta,
        head_info::Options {
            stack_commit_limit: 0,
        },
    )?;

    insta::assert_debug_snapshot!(&info, @r#"
    HeadInfo {
        stacks: [
            Stack {
                index: 0,
                tip: Some(
                    Sha1(b5743a3aa79957bcb7f654d7d4ad11d995ad5303),
                ),
                segments: [
                    StackSegment {
                        ref_name: Some(
                            FullName(
                                "refs/heads/main",
                            ),
                        ),
                        ref_location: Some(
                            AtHead,
                        ),
                        commits_unique_from_tip: [
                            BranchCommit {
                                id: Sha1(b5743a3aa79957bcb7f654d7d4ad11d995ad5303),
                                title: "10\n",
                                committed_date: Time {
                                    seconds: 946771200,
                                    offset: 0,
                                    sign: Plus,
                                },
                            },
                        ],
                        commits_unintegratd_local: [],
                        commits_unintegrated_upstream: [],
                        remote_tracking_ref_name: None,
                        metadata: None,
                    },
                    StackSegment {
                        ref_name: Some(
                            FullName(
                                "refs/heads/nine",
                            ),
                        ),
                        ref_location: Some(
                            AtHead,
                        ),
                        commits_unique_from_tip: [
                            BranchCommit {
                                id: Sha1(344e3209e344c1eb90bedb4b00b4d4999a84406c),
                                title: "9\n",
                                committed_date: Time {
                                    seconds: 946771200,
                                    offset: 0,
                                    sign: Plus,
                                },
                            },
                            BranchCommit {
                                id: Sha1(599c271e8734e58a96b3a22666704c2a72623f7f),
                                title: "8\n",
                                committed_date: Time {
                                    seconds: 946771200,
                                    offset: 0,
                                    sign: Plus,
                                },
                            },
                            BranchCommit {
                                id: Sha1(05f069b1c601c098170571bc9fab6966606f8b51),
                                title: "7\n",
                                committed_date: Time {
                                    seconds: 946771200,
                                    offset: 0,
                                    sign: Plus,
                                },
                            },
                        ],
                        commits_unintegratd_local: [],
                        commits_unintegrated_upstream: [],
                        remote_tracking_ref_name: None,
                        metadata: None,
                    },
                    StackSegment {
                        ref_name: Some(
                            FullName(
                                "refs/heads/six",
                            ),
                        ),
                        ref_location: Some(
                            AtHead,
                        ),
                        commits_unique_from_tip: [
                            BranchCommit {
                                id: Sha1(c4f2a356d6ed7250bab3dd7c58e1922b95f288c5),
                                title: "6\n",
                                committed_date: Time {
                                    seconds: 946771200,
                                    offset: 0,
                                    sign: Plus,
                                },
                            },
                            BranchCommit {
                                id: Sha1(44c12cef1e9fa109b2516079cb1f849049af3cbf),
                                title: "5\n",
                                committed_date: Time {
                                    seconds: 946771200,
                                    offset: 0,
                                    sign: Plus,
                                },
                            },
                            BranchCommit {
                                id: Sha1(c584dbe79d5ef9d630d006957b3b657cee1e80df),
                                title: "4\n",
                                committed_date: Time {
                                    seconds: 946771200,
                                    offset: 0,
                                    sign: Plus,
                                },
                            },
                        ],
                        commits_unintegratd_local: [],
                        commits_unintegrated_upstream: [],
                        remote_tracking_ref_name: None,
                        metadata: None,
                    },
                    StackSegment {
                        ref_name: Some(
                            FullName(
                                "refs/heads/three",
                            ),
                        ),
                        ref_location: Some(
                            AtHead,
                        ),
                        commits_unique_from_tip: [
                            BranchCommit {
                                id: Sha1(281da9454d5b41844d28e453e80b24925a7c8c7a),
                                title: "3\n",
                                committed_date: Time {
                                    seconds: 946771200,
                                    offset: 0,
                                    sign: Plus,
                                },
                            },
                            BranchCommit {
                                id: Sha1(12995d783f3ac841a1774e9433ee8e4c1edac576),
                                title: "2\n",
                                committed_date: Time {
                                    seconds: 946771200,
                                    offset: 0,
                                    sign: Plus,
                                },
                            },
                        ],
                        commits_unintegratd_local: [],
                        commits_unintegrated_upstream: [],
                        remote_tracking_ref_name: None,
                        metadata: None,
                    },
                    StackSegment {
                        ref_name: Some(
                            FullName(
                                "refs/heads/one",
                            ),
                        ),
                        ref_location: Some(
                            AtHead,
                        ),
                        commits_unique_from_tip: [
                            BranchCommit {
                                id: Sha1(3d57fc18d679a1ba45bc7f79e394a5e2606719ee),
                                title: "1\n",
                                committed_date: Time {
                                    seconds: 946771200,
                                    offset: 0,
                                    sign: Plus,
                                },
                            },
                        ],
                        commits_unintegratd_local: [],
                        commits_unintegrated_upstream: [],
                        remote_tracking_ref_name: None,
                        metadata: None,
                    },
                ],
                stash_status: None,
            },
        ],
        target_ref: None,
    }
    "#);

    assert_eq!(info.stacks[0].segments.len(), 5, "multiple segments");
    Ok(())
}

mod utils {
    use but_workspace::VirtualBranchesTomlMetadata;

    pub fn read_only_in_memory_scenario(
        name: &str,
    ) -> anyhow::Result<(
        gix::Repository,
        std::mem::ManuallyDrop<VirtualBranchesTomlMetadata>,
    )> {
        let repo = crate::utils::read_only_in_memory_scenario(name)?;
        let meta = VirtualBranchesTomlMetadata::from_path(
            repo.path()
                .join(".git")
                .join("should-never-be-written.toml"),
        )?;
        Ok((repo, std::mem::ManuallyDrop::new(meta)))
    }
}

#[test]
fn instantiation() -> anyhow::Result<()> {
    let repo = conflict_repo("normal-and-artificial")?;
    let normal = but_core::Commit::from_id(repo.rev_parse_single("normal")?)?;
    assert!(!normal.is_conflicted());

    let conflicted = but_core::Commit::from_id(repo.rev_parse_single("conflicted")?)?;
    assert!(conflicted.is_conflicted());
    Ok(())
}

#[test]
fn changes_between_conflicted_and_normal_commit() -> anyhow::Result<()> {
    let repo = conflict_repo("normal-and-artificial")?;
    let changes = but_core::diff::commit_to_commit(
        &repo,
        Some(repo.rev_parse_single("normal")?.into()),
        repo.rev_parse_single("conflicted")?.into(),
    )?;
    insta::assert_debug_snapshot!(changes, @r#"
        [
            TreeChange {
                path: "file",
                status: Modification {
                    previous_state: ChangeState {
                        id: Sha1(d95f3ad14dee633a758d2e331151e950dd13e4ed),
                        kind: Blob,
                    },
                    state: ChangeState {
                        id: Sha1(e69de29bb2d1d6434b8b29ae775ad8c2e48c5391),
                        kind: Blob,
                    },
                    flags: None,
                },
            },
        ]
        "#);
    Ok(())
}
#[test]

fn changes_between_conflicted_and_conflicted_commit() -> anyhow::Result<()> {
    let repo = conflict_repo("normal-and-artificial")?;
    let changes = but_core::diff::commit_to_commit(
        &repo,
        Some(repo.rev_parse_single("conflicted")?.into()),
        repo.rev_parse_single("conflicted")?.into(),
    )?;
    insta::assert_debug_snapshot!(changes, @"[]");
    Ok(())
}

fn conflict_repo(name: &str) -> anyhow::Result<gix::Repository> {
    let root = gix_testtools::scripted_fixture_read_only("conflict-commits.sh")
        .map_err(anyhow::Error::from_boxed)?;
    Ok(gix::open_opts(
        root.join(name),
        gix::open::Options::isolated(),
    )?)
}

use but_api::branch::new_only;
use but_testsupport::Sandbox;

#[test]
fn new_basic() -> anyhow::Result<()> {
    let env = Sandbox::simulate_clone()?;
    let repo = env.open_repo()?;
    // TODO: `rev_parse_single` works (which is good) but the commit is not
    // found in the rebase editor (presumably because it's an upstream commit).
    // To create a new branch with no local commits, I think that we need to be
    // able to refer to it (i.e. the rebase editor must know about it).
    let result = new_only(
        &env.context()?,
        gix::refs::FullName::try_from("refs/heads/branch_name")?,
        repo.rev_parse_single(b"remotes/origin/main")?.object()?.id,
    );
    insta::assert_debug_snapshot!(result, @r#"
    Err(
        "Failed to find commit cfb66c875f10ef8efa5036caef7e6fe09bd1aee8 in rebase editor",
    )
    "#);
    Ok(())
}

use bstr::ByteSlice;
use but_core::branch::refuse_colliding_refname;
use but_testsupport::read_only_in_memory_scenario_named_with_post;
use gix::refs::transaction::PreviousValue;

#[test]
fn names_that_only_read_alike_do_not_collide() -> anyhow::Result<()> {
    let (repo, _) =
        read_only_in_memory_scenario_named_with_post("unborn-empty", "", 1, |fixture| {
            let repo = but_testsupport::open_repo(fixture.path())?;
            let id = repo.object_hash().null();
            for name in ["refs/heads/a-\u{FFFD}/inner", "refs/heads/b-\u{FFFD}"] {
                let name: &gix::refs::FullNameRef = name.try_into()?;
                repo.reference(name, id, PreviousValue::Any, "test")?;
            }
            Ok(())
        })?;

    for name in [b"refs/heads/a-\xff".as_slice(), b"refs/heads/b-\xff/inner"] {
        let name: &gix::refs::FullNameRef = name.as_bstr().try_into()?;
        refuse_colliding_refname(&repo, name, None)?;
    }
    Ok(())
}

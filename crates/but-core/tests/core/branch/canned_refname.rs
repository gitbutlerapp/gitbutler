use but_core::branch::canned_refname;
use but_testsupport::read_only_in_memory_scenario;

#[test]
fn latin_ascii() -> anyhow::Result<()> {
    let repo = read_only_in_memory_scenario("unborn-empty")?;
    assert_eq!(canned_refname(&repo)?.shorten(), "amo-branch-1");
    assert_eq!(
        canned_refname(&repo)?.shorten(),
        "amo-branch-1",
        "it produces the same name again"
    );
    Ok(())
}

#[test]
fn latin_with_accents() -> anyhow::Result<()> {
    let mut repo = read_only_in_memory_scenario("unborn-empty")?;
    set_author(&mut repo, "Étienne Çhaplin")?;
    assert_eq!(
        canned_refname(&repo)?.shorten(),
        "éç-branch-1",
        "it now picks accented characters"
    );
    Ok(())
}

#[test]
fn chinese() -> anyhow::Result<()> {
    let mut repo = read_only_in_memory_scenario("unborn-empty")?;
    set_author(&mut repo, "冯宇雷")?;
    assert_eq!(
        canned_refname(&repo)?.shorten(),
        "冯宇雷-branch-1",
        "it can use up to 3 CJK characters"
    );
    Ok(())
}

#[test]
fn emoji() -> anyhow::Result<()> {
    let mut repo = read_only_in_memory_scenario("unborn-empty")?;
    set_author(&mut repo, "😁🤦‍♂️")?;
    assert_eq!(
        canned_refname(&repo)?.shorten(),
        "branch-1",
        "only emojies aren't allowed"
    );
    Ok(())
}

#[test]
fn prefixed_emoji_in_latin_name() -> anyhow::Result<()> {
    let mut repo = read_only_in_memory_scenario("unborn-empty")?;
    set_author(&mut repo, "🩷Harry Awesome🤦‍♂️")?;
    assert_eq!(
        canned_refname(&repo)?.shorten(),
        "ha-branch-1",
        "emojies are skipped"
    );
    Ok(())
}

#[test]
fn arabic_left_to_right() -> anyhow::Result<()> {
    let mut repo = read_only_in_memory_scenario("unborn-empty")?;
    set_author(&mut repo, "فهد بنجستن")?;
    assert_eq!(
        canned_refname(&repo)?.shorten(),
        "branch-1",
        "it can't do it at all right now"
    );
    Ok(())
}

#[test]
fn no_author_configured() -> anyhow::Result<()> {
    let mut repo = read_only_in_memory_scenario("unborn-empty")?;
    {
        let mut config = repo.config_snapshot_mut();
        config.raw_values_mut(&"author.name")?.delete_all();
        config.raw_values_mut(&"author.email")?.delete_all();
    }

    assert_eq!(
        canned_refname(&repo)?.shorten(),
        "branch-1",
        "it doesn't use a prefix"
    );
    Ok(())
}

fn set_author(repo: &mut gix::Repository, name: &str) -> anyhow::Result<()> {
    let mut config = repo.config_snapshot_mut();
    config.set_raw_value(&"author.name", name)?;
    Ok(())
}

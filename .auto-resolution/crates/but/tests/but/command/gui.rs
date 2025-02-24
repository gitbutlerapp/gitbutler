use crate::utils::Sandbox;
use snapbox::str;

#[test]
fn good_error_message_for_non_directories() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    env.file("not-a-dir", "content-does-not-matter");

    env.but("not-a-dir")
        .assert()
        .failure()
        .stdout_eq(str![[]])
        .stderr_eq(str![[r#"
Error: Can only open the GUI on directories: './not-a-dir'

"#]]);
    Ok(())
}

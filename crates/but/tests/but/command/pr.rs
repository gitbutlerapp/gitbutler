use snapbox::str;

use crate::utils::Sandbox;

#[test]
fn new_rejects_a_missing_attachment_before_pushing() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    // Rejected before forge auth or a push, so nothing leaves the machine.
    env.but("pr new A -m Title --attach missing.png#Alt")
        .assert()
        .failure()
        .stdout_eq(str![])
        .stderr_eq(str![[r#"
Error: Bad input 'missing.png#Alt' for '--attach'

'missing.png' is not a file.

"#]]);
}

#[test]
fn new_rejects_attachments_when_signed_out() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env.file("shot.png", "png");

    // The sandbox has no GitButler account, and uploads need one.
    env.but("pr new A -m Title --attach shot.png")
        .assert()
        .failure()
        .stdout_eq(str![])
        .stderr_eq(str![[r#"
Error: Bad input for '--attach'

Attaching files needs a GitButler account, and none is signed in.

Hint: Sign in to GitButler in the desktop app, then run this again.

"#]]);
}

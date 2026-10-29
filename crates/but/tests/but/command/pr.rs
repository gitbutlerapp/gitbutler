use crate::utils::Sandbox;

#[test]
fn new_message_file_missing_is_bad_input() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    // The message file is read before the forge is contacted.
    env.but("pr new A -F nope.txt")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Bad input for '--file'

Could not read message from 'nope.txt': [..]

"#]]);
}

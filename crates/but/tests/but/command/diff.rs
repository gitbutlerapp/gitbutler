use crate::utils::Sandbox;

#[test]
fn path_prefix() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");

    env.setup_metadata(&["A", "B"]);
    env.file("prefixx", "don't show this\n");
    env.file("prefix/a", "we want this\n");
    env.file("prefix/b", "we also want this\n");
    env.but("diff prefix/")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
───────────╮
n0 prefix/a│
───────────╯
     1│+we want this
───────────╮
m0 prefix/b│
───────────╯
     1│+we also want this

"#]]);
}

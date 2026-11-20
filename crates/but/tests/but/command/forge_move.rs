use snapbox::str;

use crate::utils::Sandbox;

#[test]
fn moving_without_an_account_and_terminal_names_how_to_sign_in() {
    let env = Sandbox::open_with_default_settings("repo-with-remote-and-head");

    // Signing in needs a browser and a person, so a non-interactive run stops before changing
    // anything and says who has to do what.
    env.but("config forge _move")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Not signed in to GitButler Code

Hint: Run `but config forge auth` in a terminal and pick GitButler Code (an agent has to ask the user to run it)

"#]]);
}

#[test]
fn pushing_in_a_clone_of_an_organization_project_needs_an_account() {
    let env = Sandbox::open_with_default_settings("repo-with-remote-and-head");
    env.invoke_git("remote set-url origin https://code.gitbutler.com/gitbutler.com/demo.git");

    // The organization's project takes only `main`, so branches go to the user's copy, which
    // `but push` sets up with their account. With none, it stops before pushing anything.
    env.but("push")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Not signed in to GitButler Code

Hint: Run `but config forge auth` in a terminal and pick GitButler Code (an agent has to ask the user to run it)

"#]]);
}

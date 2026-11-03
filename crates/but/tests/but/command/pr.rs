use crate::utils::Sandbox;

#[test]
fn new_message_file_missing_is_bad_input() {
    let env = sandbox();

    // The message file is read before the forge is contacted.
    env.but("pr new A -F nope.txt")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Bad input for '--file'

Could not read message from 'nope.txt': [..]

"#]]);
}

#[test]
fn new_without_title_is_bad_input() {
    let env = sandbox();

    env.but("pr new A -m ''")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Aborting due to empty PR title

Hint: Put the title on the first line of the message, or use `-t` for the default title and description

"#]]);
}

#[test]
fn new_without_remote_is_bad_input() {
    let env = sandbox();
    env.invoke_git("remote remove origin");

    env.but("pr new A -m title")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: The remote named "origin" did not exist[..]

Hint: Run `but config push-remote <name>`, first adding the remote with `git remote add <name> <url>` if it is missing, then retry

"#]]);
}

#[test]
fn new_on_unknown_forge_is_bad_input() {
    let env = sandbox();
    env.invoke_git("remote set-url origin https://git.example.com/o/r.git");

    env.but("pr new A -m title")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Unable to determine the forge for this project. Is target branch associated with a supported forge?

Hint: If this is a self-hosted GitHub or GitLab, run `but config forge auth` in a terminal (an agent has to ask the user to run it). Otherwise, run `but push <branch>` and open the review on the forge's website

"#]]);
}

#[test]
fn new_on_azure_is_bad_input() {
    let env = sandbox();
    env.invoke_git("remote set-url origin https://dev.azure.com/o/p/_git/r");

    env.but("pr new A -m title")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Azure is unsupported at the minute. Sorry 😞.

Hint: Run `but push <branch>` and open the review on the Azure DevOps website

"#]]);
}

#[test]
fn new_without_forge_account_is_bad_input() {
    let env = sandbox();
    env.invoke_git("remote set-url origin https://github.com/o/r.git");

    env.but("pr new A -m title")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: No authenticated forge users found.

Hint: Run `but config forge auth` in a terminal to authenticate with GitHub, then retry. It prompts for input, so an agent has to ask the user to run it

"#]]);
}

#[test]
fn new_skips_blank_lines_before_the_title() {
    let env = sandbox();
    env.invoke_git("remote set-url origin https://github.com/o/r.git");

    // Getting as far as the account check means the message has a title.
    env.but("pr new A -m '\n\ntitle'")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: No authenticated forge users found.
...
"#]]);
}

fn sandbox() -> Sandbox {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env
}

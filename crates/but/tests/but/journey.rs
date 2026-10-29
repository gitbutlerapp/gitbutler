//! Tests for various nice user-journeys, from different starting points, performing multiple common steps in sequence.

#[cfg(feature = "legacy")]
use crate::utils::Sandbox;

#[cfg(feature = "legacy")]
#[test]
fn from_workspace() {
    use snapbox::IntoData;
    use snapbox::file;

    use crate::utils::CommandExt;
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    snapbox::assert_data_eq!(
        env.git_log(),
        snapbox::str![[r#"
*   c128bce (HEAD -> gitbutler/workspace) GitButler Workspace Commit
|\  
| * 9477ae7 (A) add A
* | d3e2ba3 (B) add B
|/  
* 0dc3733 (origin/main, origin/HEAD, main) add M

"#]]
        .raw()
    );
    snapbox::assert_data_eq!(env.git_status(), snapbox::str![""]);

    // Must set metadata to match the scenario, or else the old APIs used here won't deliver.
    env.setup_metadata(&["A", "B"]);

    env.but("status")
        .with_color_for_svg()
        .assert()
        .success()
        .stdout_eq(file!["snapshots/from-workspace/status01.stdout.term.svg"]);

    env.but("status -v")
        .with_color_for_svg()
        .assert()
        .success()
        .stdout_eq(file![
            "snapshots/from-workspace/status01-verbose.stdout.term.svg"
        ]);

    // List is the default
    env.but("branch")
        .with_color_for_svg()
        .assert()
        .success()
        .stdout_eq(file!["snapshots/from-workspace/branch01.stdout.term.svg"]);

    // But list is also explicit.
    env.but("branch list")
        .with_color_for_svg()
        .assert()
        .success()
        .stdout_eq(file!["snapshots/from-workspace/branch01.stdout.term.svg"]);

    // TODO: more operations on the repository!
}

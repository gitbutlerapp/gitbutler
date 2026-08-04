use std::path::PathBuf;

use super::*;

#[test]
fn intro_explains_wizard_flow_before_first_prompt() {
    let mut intro = String::new();

    write_intro(&mut intro, None).expect("write intro");

    assert!(intro.contains("GitButler · agent setup"));
    assert!(intro.contains("Set up your coding agent to work well with GitButler"));
    assert!(intro.contains("Install the GitButler skill"));
    assert!(intro.contains("commits, branches, and opens PRs"));
    assert!(intro.contains("Nothing is written until you review and confirm"));
    assert!(intro.contains("see exactly what changes first"));
    assert!(intro.contains("No repository here"));
}

#[test]
fn scope_options_lead_with_global_and_name_repo() {
    let repo = RepoInfo {
        root: PathBuf::from("/tmp/gitbutler"),
        needs_setup: false,
    };

    let options = scope_options(&repo);
    let labels = options
        .iter()
        .map(|(label, scope)| (label.label.as_str(), *scope))
        .collect::<Vec<_>>();

    // Global leads so it is the highlighted default; the repo is still named.
    assert_eq!(
        labels,
        vec![
            ("All my projects (global)", Scope::Global),
            ("Just this project (gitbutler)", Scope::Repository),
            ("Both", Scope::Both),
        ]
    );
}

#[test]
fn display_path_strips_leading_dot_component() {
    use std::path::MAIN_SEPARATOR as SEP;
    // A leading `.` component is dropped; the rest keeps native separators
    // (`./` on POSIX, `.\` on Windows), so build paths and expectations from
    // components rather than hard-coding `/`.
    assert_eq!(
        display_path(&PathBuf::from(".").join("AGENTS.md")),
        "AGENTS.md"
    );
    let nested = PathBuf::from(".")
        .join(".codex")
        .join("skills")
        .join("gitbutler");
    assert_eq!(
        display_path(&nested),
        format!(".codex{SEP}skills{SEP}gitbutler")
    );
}

#[test]
fn display_path_collapses_home_to_tilde() {
    use std::path::MAIN_SEPARATOR as SEP;
    let home = but_path::home_dir().expect("home dir");
    let inside = home.join(".codex").join("AGENTS.md");
    assert_eq!(display_path(&inside), format!("~{SEP}.codex{SEP}AGENTS.md"));
}

#[test]
fn repo_display_name_resolves_dot_to_current_folder_name() {
    let repo = RepoInfo {
        root: PathBuf::from("."),
        needs_setup: false,
    };
    let expected =
        display_name_from_path(&std::env::current_dir().expect("current dir has a folder name"))
            .expect("current dir has a folder name");

    assert_eq!(repo_display_name(&repo), expected);
}

#[test]
fn workflow_row_disables_repo_local_option_outside_single_repo() {
    // Outside a single-repo setup the rule would also land in the global config,
    // so the option is offered disabled. The label is unchanged in every scope;
    // the grayed row and its help (not a label suffix) carry the meaning.
    let label = WorkflowOption::PushToTarget.label();
    for scope in [Scope::Global, Scope::Both] {
        let (row_label, help, disabled) = workflow_option_row(WorkflowOption::PushToTarget, scope);
        assert!(disabled, "PushToTarget must be disabled for {scope:?}");
        assert_eq!(row_label, label);
        assert!(help.contains("Just this project"));
    }

    let (row_label, _help, disabled) =
        workflow_option_row(WorkflowOption::PushToTarget, Scope::Repository);
    assert!(
        !disabled,
        "PushToTarget must be selectable for a single repo"
    );
    assert_eq!(row_label, label);
}

#[test]
fn workflow_row_keeps_non_repo_local_options_enabled_in_every_scope() {
    for scope in [Scope::Global, Scope::Repository, Scope::Both] {
        let (label, help, disabled) = workflow_option_row(WorkflowOption::DraftPrs, scope);
        assert!(!disabled);
        assert_eq!(label, WorkflowOption::DraftPrs.label());
        assert_eq!(help, WorkflowOption::DraftPrs.help());
    }
}

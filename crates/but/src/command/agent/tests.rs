use std::path::PathBuf;

use super::{
    files::{find_line_anchored, upsert_managed_block, upsert_managed_block_file},
    plan::{collect_instruction_writes, collect_skill_installs, repository_setup_needed},
    *,
};

#[test]
fn devin_uses_shared_agent_skills_target() {
    assert_eq!(
        AgentTarget::from_detected(detect_agent::Agent::Devin),
        Some(AgentTarget::AgentSkills)
    );
}

#[test]
fn dirac_uses_shared_agent_skills_target() {
    assert_eq!(
        AgentTarget::from_detected(detect_agent::Agent::Dirac),
        Some(AgentTarget::AgentSkills)
    );
}

#[test]
fn generated_default_policy_includes_baseline_and_default_preferences() {
    let policy = render_managed_policy_block(&WizardAnswers::default());

    assert!(policy.contains("## Version control"));
    assert!(
        policy
            .contains("Use GitButler (`but`) for version-control inspection and write operations")
    );
    assert!(policy.contains("otherwise modify another agent's work"));
    assert!(policy.contains("For commit just/only/specific changes on a new branch"));
    assert!(policy.contains("For that fast path, after the commit succeeds, stop and summarize"));
    assert!(policy.contains("Use the installed GitButler skill for command recipes and syntax"));
    assert!(policy.contains("amend an unpublished local commit"));
    assert!(policy.contains("Use GitButler to move the relevant changes"));
    assert!(policy.contains("If one file contains unrelated changes"));
    assert!(!policy.contains("ship it"));
}

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
fn agent_in_use_detects_home_config_dir() {
    let home = tempfile::tempdir().expect("home");
    std::fs::create_dir_all(home.path().join(".codex")).expect("create ~/.codex");

    assert!(
        AgentTarget::Codex.in_use(Some(home.path()), None),
        "a ~/.codex directory should mark Codex as in use"
    );
    assert!(!AgentTarget::OpenCode.in_use(Some(home.path()), None));
    assert!(!AgentTarget::Codex.in_use(None, None));
}

#[test]
fn agent_in_use_detects_repo_marker_but_ignores_shared_agents_md() {
    let dir = tempfile::tempdir().expect("repo");
    std::fs::write(dir.path().join("CLAUDE.md"), "x").expect("write CLAUDE.md");
    std::fs::write(dir.path().join("AGENTS.md"), "x").expect("write AGENTS.md");
    let repo = RepoInfo {
        root: dir.path().to_path_buf(),
        needs_setup: false,
    };

    assert!(AgentTarget::ClaudeCode.in_use(None, Some(&repo)));
    // AGENTS.md is shared by several agents, so it is not a marker.
    assert!(!AgentTarget::Codex.in_use(None, Some(&repo)));
}

#[test]
fn agent_in_use_detects_existing_skill_install() {
    let home = tempfile::tempdir().expect("home");
    // A skill a prior run installed for OpenCode
    // should mark it in use even without other config present.
    std::fs::create_dir_all(
        home.path()
            .join(".config")
            .join("opencode")
            .join("skills")
            .join("gitbutler"),
    )
    .expect("create skill dir");

    assert!(AgentTarget::OpenCode.in_use(Some(home.path()), None));
    assert!(!AgentTarget::Codex.in_use(Some(home.path()), None));
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
fn upsert_managed_block_appends_without_touching_existing_text() {
    let existing = "# Existing\n\nKeep me.\n";
    let block = format!("{MANAGED_BLOCK_START}\nnew\n{MANAGED_BLOCK_END}\n");

    let updated = upsert_managed_block(existing, &block).expect("append managed block");

    assert!(updated.starts_with(existing));
    assert!(updated.contains(&block));
}

#[test]
fn upsert_managed_block_replaces_existing_block_without_duplication() {
    let existing = format!("before\n{MANAGED_BLOCK_START}\nold\n{MANAGED_BLOCK_END}\nafter\n");
    let block = format!("{MANAGED_BLOCK_START}\nnew\n{MANAGED_BLOCK_END}\n");

    let updated = upsert_managed_block(&existing, &block).expect("replace managed block");

    assert_eq!(updated.matches(MANAGED_BLOCK_START).count(), 1);
    assert!(updated.contains("before\n"));
    assert!(updated.contains("new\n"));
    assert!(updated.contains("after\n"));
    assert!(!updated.contains("old\n"));
}

#[test]
fn upsert_managed_block_rejects_partial_block() {
    let existing = format!("before\n{MANAGED_BLOCK_START}\nold\n");
    let block = format!("{MANAGED_BLOCK_START}\nnew\n{MANAGED_BLOCK_END}\n");

    let err = upsert_managed_block(&existing, &block).expect_err("partial block fails");

    assert!(err.to_string().contains("partial managed block"));
}

#[test]
fn push_to_target_is_the_only_repo_local_option() {
    for option in WorkflowOption::ALL {
        assert_eq!(
            option.repo_local_only(),
            option == WorkflowOption::PushToTarget,
            "{option:?} repo_local_only mismatch"
        );
    }
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

#[test]
fn default_policy_omits_land_section_until_selected() {
    let default = render_managed_policy_block(&WizardAnswers::default());
    assert!(!default.contains("skip-the-PR workflow"));

    let answers = WizardAnswers {
        selected: vec![WorkflowOption::PushToTarget],
        ..WizardAnswers::default()
    };
    let policy = render_managed_policy_block(&answers);
    assert!(policy.contains("### Skip pull requests and land onto the target"));
    assert!(policy.contains("`but land <branch>`"));
    assert!(policy.contains("takes precedence"));
    assert!(policy.contains("must pass `--yes` to confirm"));
}

#[test]
fn publish_phrase_opens_pull_request_without_push_to_target() {
    let answers = WizardAnswers {
        selected: vec![WorkflowOption::PublishPhrase],
        ..WizardAnswers::default()
    };
    let policy = render_managed_policy_block(&answers);
    assert!(policy.contains("open or update its pull request"));
    assert!(!policy.contains("but land"));
}

#[test]
fn push_to_target_supersedes_publish_phrase_pull_requests() {
    let answers = WizardAnswers {
        selected: vec![WorkflowOption::PublishPhrase, WorkflowOption::PushToTarget],
        publish_phrase: "ship it".to_string(),
        ..WizardAnswers::default()
    };
    let policy = render_managed_policy_block(&answers);
    // The publish phrase lands onto the target instead of opening a PR, and the
    // skip-the-PR section declares it supersedes the other PR-based rules.
    assert!(policy.contains("When the user says `ship it`"));
    assert!(policy.contains("land that branch onto the target with `but land <branch> --yes`"));
    assert!(!policy.contains("open or update its pull request"));
    assert!(policy.contains("takes precedence"));
}

#[test]
fn generated_policy_includes_selected_custom_values() {
    let answers = WizardAnswers {
        selected: vec![
            WorkflowOption::PublishPhrase,
            WorkflowOption::BranchPattern,
            WorkflowOption::CommitConvention,
        ],
        publish_phrase: "release this".to_string(),
        branch_pattern: Some("<name>/<ticket>-<short-description>".to_string()),
        commit_convention: Some("type(scope): summary".to_string()),
    };

    let policy = render_managed_policy_block(&answers);

    assert!(policy.contains("`release this`"));
    assert!(policy.contains("`<name>/<ticket>-<short-description>`"));
    assert!(policy.contains("`type(scope): summary`"));
}

#[test]
fn generated_policy_expands_selected_tuning_recipes() {
    let answers = WizardAnswers {
        selected: WorkflowOption::ALL.to_vec(),
        publish_phrase: "release this".to_string(),
        branch_pattern: Some("<name>/<short-description>".to_string()),
        commit_convention: Some("type(scope): summary".to_string()),
    };

    let policy = render_managed_policy_block(&answers);

    assert!(policy.contains("Do not create tiny fixup commits"));
    assert!(policy.contains("Keep tests with the behavior they verify"));
    assert!(policy.contains("Use `but move` for branch stacking and restacking"));
    assert!(policy.contains("create pull requests with `but pr`, not `gh`"));
    assert!(policy.contains("update with `but pull` directly"));
    assert!(policy.contains("run `but pull --check` first and ask before updating"));
    assert!(policy.contains("create it as a draft with GitButler"));
    assert!(
        policy
            .contains("land the session branch directly onto the target with `but land <branch>`")
    );
    assert!(policy.contains("When the user says `release this`"));
    // With push-to-target also selected, the publish phrase lands instead of
    // opening a pull request.
    assert!(policy.contains("land that branch onto the target with `but land <branch> --yes`"));
    assert!(policy.contains("When creating a GitButler branch for an agent session"));
    assert!(policy.contains("commit-message convention"));
    assert!(policy.contains("### Commit checkpoints after each turn"));
    assert!(policy.contains("checkpoint commits as local savepoints"));
    assert!(policy.contains("squash commits, reword commits, and move changes"));
}

#[test]
fn instruction_writes_group_shared_agents_by_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let repo = RepoInfo {
        root: dir.path().to_path_buf(),
        needs_setup: false,
    };

    let (writes, notes) = collect_instruction_writes(
        &[
            AgentTarget::Codex,
            AgentTarget::OpenCode,
            AgentTarget::AgentSkills,
            AgentTarget::Windsurf,
        ],
        Scope::Repository,
        Some(&repo),
    )
    .expect("collect instruction writes");

    assert!(notes.is_empty());
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].path, repo.root.join("AGENTS.md"));
    assert_eq!(
        writes[0].agents,
        vec![
            AgentTarget::Codex,
            AgentTarget::OpenCode,
            AgentTarget::AgentSkills,
            AgentTarget::Windsurf
        ]
    );
}

#[test]
fn skill_installs_expand_both_scopes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let repo = RepoInfo {
        root: dir.path().to_path_buf(),
        needs_setup: false,
    };

    let installs = collect_skill_installs(&[AgentTarget::Codex], Scope::Both, Some(&repo))
        .expect("collect skill installs");

    assert_eq!(installs.len(), 2);
    let repo_path = repo.root.join(".codex").join("skills").join("gitbutler");
    assert!(installs.iter().any(|install| install.path == repo_path));
    assert!(installs.iter().any(|install| {
        install.path != repo_path && install.path.ends_with(".codex/skills/gitbutler")
    }));
}

/// Count the number of line-anchored block start markers.
fn anchored_start_count(text: &str) -> usize {
    let mut count = 0;
    let mut pos = 0;
    while let Some(idx) = find_line_anchored(text, MANAGED_BLOCK_START, pos) {
        count += 1;
        pos = idx + MANAGED_BLOCK_START.len();
    }
    count
}

#[test]
fn upsert_managed_block_ignores_marker_quoted_in_prose() {
    // A marker mentioned inside inline code (not on its own line) must not be
    // treated as a block delimiter, or the splice would delete prose.
    let existing = format!(
        "# Notes\n\nWe use the `{MANAGED_BLOCK_START}` marker convention.\n\nKeep this paragraph.\n\n{MANAGED_BLOCK_START}\nold\n{MANAGED_BLOCK_END}\n\nTrailing prose.\n"
    );
    let block = format!("{MANAGED_BLOCK_START}\nnew\n{MANAGED_BLOCK_END}\n");

    let updated = upsert_managed_block(&existing, &block).expect("replace real block only");

    assert!(updated.starts_with("# Notes"));
    assert!(updated.contains("We use the `"));
    assert!(updated.contains("Keep this paragraph."));
    assert!(updated.contains("Trailing prose."));
    assert!(updated.contains("new\n"));
    assert!(!updated.contains("\nold\n"));
    assert_eq!(anchored_start_count(&updated), 1);
}

#[test]
fn upsert_managed_block_ignores_marker_inside_fenced_code() {
    // A file documenting the markers inside a fenced code block must be left
    // intact, not spliced — the real block is appended after it.
    let existing = format!(
        "# Docs\n\nExample:\n\n```md\n{MANAGED_BLOCK_START}\n## example\n{MANAGED_BLOCK_END}\n```\n\nKeep me.\n"
    );
    let block = format!("{MANAGED_BLOCK_START}\nnew\n{MANAGED_BLOCK_END}\n");

    let updated = upsert_managed_block(&existing, &block).expect("append, not splice");

    assert!(updated.contains("## example"), "fenced example preserved");
    assert!(updated.contains("Keep me."));
    assert!(updated.contains("new\n"));
    // The example markers (inside the fence) are ignored, so a fresh block is
    // appended: the example pair plus the new pair.
    assert_eq!(updated.matches(MANAGED_BLOCK_START).count(), 2);
}

#[test]
fn upsert_managed_block_dedups_multiple_blocks() {
    let existing = format!(
        "head\n\n{MANAGED_BLOCK_START}\nA\n{MANAGED_BLOCK_END}\n\nmiddle\n\n{MANAGED_BLOCK_START}\nB\n{MANAGED_BLOCK_END}\n\ntail\n"
    );
    let block = format!("{MANAGED_BLOCK_START}\nnew\n{MANAGED_BLOCK_END}\n");

    let updated = upsert_managed_block(&existing, &block).expect("collapse to one block");

    assert_eq!(updated.matches(MANAGED_BLOCK_START).count(), 1);
    assert_eq!(updated.matches(MANAGED_BLOCK_END).count(), 1);
    assert!(updated.contains("new\n"));
    assert!(!updated.contains("\nA\n"));
    assert!(!updated.contains("\nB\n"));
    assert!(updated.contains("head"));
    assert!(updated.contains("middle"));
    assert!(updated.contains("tail"));
}

#[test]
fn upsert_managed_block_rejects_reversed_markers() {
    let existing = format!("{MANAGED_BLOCK_END}\nstray\n{MANAGED_BLOCK_START}\n");
    let block = format!("{MANAGED_BLOCK_START}\nnew\n{MANAGED_BLOCK_END}\n");

    let err = upsert_managed_block(&existing, &block).expect_err("reversed markers fail");

    assert!(err.to_string().contains("wrong order"));
}

#[test]
fn upsert_managed_block_appends_with_crlf_separator() {
    let existing = "# Title\r\n\r\nKeep me.\r\n";
    let block = format!("{MANAGED_BLOCK_START}\nnew\n{MANAGED_BLOCK_END}\n");

    let updated = upsert_managed_block(existing, &block).expect("crlf append");

    assert!(updated.starts_with(existing));
    assert!(updated.contains(&format!(
        "{MANAGED_BLOCK_START}\r\nnew\r\n{MANAGED_BLOCK_END}"
    )));
    // No bare LF: every '\n' must be part of a "\r\n".
    assert_eq!(
        updated.matches('\n').count(),
        updated.matches("\r\n").count()
    );
}

#[test]
fn upsert_managed_block_replaces_crlf_block_without_mixed_endings() {
    let existing =
        format!("before\r\n{MANAGED_BLOCK_START}\r\nold\r\n{MANAGED_BLOCK_END}\r\nafter\r\n");
    let block = format!("{MANAGED_BLOCK_START}\nnew\n{MANAGED_BLOCK_END}\n");

    let updated = upsert_managed_block(&existing, &block).expect("crlf replace");

    assert_eq!(updated.matches(MANAGED_BLOCK_START).count(), 1);
    assert!(updated.contains("after\r\n"));
    assert!(!updated.contains("old"));
    assert_eq!(
        updated.matches('\n').count(),
        updated.matches("\r\n").count()
    );
}

#[test]
fn upsert_managed_block_file_is_idempotent_and_creates_nested_dirs() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("nested").join("dir").join("AGENTS.md");
    let block = render_managed_policy_block(&WizardAnswers::default());

    upsert_managed_block_file(&path, &block).expect("first upsert");
    assert!(path.exists(), "nested parent dirs must be created");
    let first = std::fs::read_to_string(&path).expect("read first");

    upsert_managed_block_file(&path, &block).expect("second upsert");
    let second = std::fs::read_to_string(&path).expect("read second");
    assert_eq!(first, second, "rerun must be byte-stable");
    assert_eq!(second.matches(MANAGED_BLOCK_START).count(), 1);
}

#[test]
fn upsert_managed_block_file_preserves_preexisting_content() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join(".github").join("copilot-instructions.md");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("create dir");
    std::fs::write(&path, "# Team rules\n\nKeep me.\n").expect("seed file");
    let block = render_managed_policy_block(&WizardAnswers::default());

    upsert_managed_block_file(&path, &block).expect("first");
    upsert_managed_block_file(&path, &block).expect("second");

    let content = std::fs::read_to_string(&path).expect("read");
    assert!(content.starts_with("# Team rules"));
    assert!(content.contains("Keep me."));
    assert_eq!(content.matches(MANAGED_BLOCK_START).count(), 1);
}

#[test]
fn skill_installs_copilot_repo_and_global_diverge() {
    let dir = tempfile::tempdir().expect("tempdir");
    let repo = RepoInfo {
        root: dir.path().to_path_buf(),
        needs_setup: false,
    };

    let installs = collect_skill_installs(&[AgentTarget::GitHubCopilot], Scope::Both, Some(&repo))
        .expect("collect skill installs");

    assert_eq!(installs.len(), 2);
    assert!(installs.iter().any(|install| {
        install.path == repo.root.join(".github").join("skills").join("gitbutler")
    }));
    assert!(
        installs
            .iter()
            .any(|install| install.path.ends_with(".copilot/skills/gitbutler"))
    );
}

#[test]
fn poolside_skill_paths_follow_scope() {
    assert_eq!(
        AgentTarget::Poolside.skill_path_components(Scope::Repository),
        Some(&[".poolside", "skills", "but"][..])
    );
    assert_eq!(
        AgentTarget::Poolside.skill_path_components(Scope::Global),
        Some(&[".config", "poolside", "skills", "but"][..])
    );
}

#[test]
fn skill_installs_global_only_uses_home_paths() {
    let installs = collect_skill_installs(
        &[
            AgentTarget::Codex,
            AgentTarget::GitHubCopilot,
            AgentTarget::OpenCode,
            AgentTarget::Windsurf,
        ],
        Scope::Global,
        None,
    )
    .expect("collect skill installs");

    assert_eq!(installs.len(), 4);
    assert!(
        installs
            .iter()
            .any(|i| i.path.ends_with(".codex/skills/gitbutler"))
    );
    assert!(
        installs
            .iter()
            .any(|i| i.path.ends_with(".copilot/skills/gitbutler"))
    );
    assert!(
        installs
            .iter()
            .any(|i| { i.path.ends_with(".config/opencode/skills/gitbutler") })
    );
    assert!(
        installs
            .iter()
            .any(|i| { i.path.ends_with(".codeium/windsurf/skills/gitbutler") })
    );
}

#[test]
fn every_agent_resolves_to_a_skill_path_for_both_scopes() {
    // Guards the AgentTarget -> SKILL_FORMATS name mapping: a typo would make an
    // agent return no install path (and silently install nothing).
    for agent in AgentTarget::ALL {
        assert!(
            agent.skill_path_components(Scope::Global).is_some(),
            "{agent:?} has no global skill path"
        );
        assert!(
            agent.skill_path_components(Scope::Repository).is_some(),
            "{agent:?} has no repository skill path"
        );
    }
}

#[test]
fn instruction_writes_global_emits_print_note_for_agents_without_global_target() {
    let (writes, notes) = collect_instruction_writes(
        &[
            AgentTarget::ClaudeCode,
            AgentTarget::Cursor,
            AgentTarget::AgentSkills,
        ],
        Scope::Global,
        None,
    )
    .expect("collect instruction writes");

    // Claude Code has a trusted global target; Cursor and Agent Skills do not.
    assert_eq!(writes.len(), 1);
    assert!(writes[0].path.ends_with(".claude/rules/gitbutler.md"));
    assert_eq!(writes[0].agents, vec![AgentTarget::ClaudeCode]);

    assert_eq!(notes.len(), 2);
    assert!(notes.iter().any(|n| n.contains("Cursor")));
    assert!(notes.iter().any(|n| n.contains("Agent Skills")));
    assert!(
        notes
            .iter()
            .all(|n| n.contains("no supported global instructions file"))
    );
}

#[test]
fn repo_scoped_planning_requires_a_repository() {
    assert!(collect_skill_installs(&[AgentTarget::Codex], Scope::Repository, None).is_err());
    assert!(collect_instruction_writes(&[AgentTarget::Codex], Scope::Repository, None,).is_err());

    // Global instructions need no repository.
    let (writes, notes) = collect_instruction_writes(&[AgentTarget::Codex], Scope::Global, None)
        .expect("global instructions need no repository");
    assert_eq!(writes.len(), 1);
    assert!(notes.is_empty());
}

#[test]
fn global_only_plan_does_not_need_repository_setup() {
    let repo = RepoInfo {
        root: PathBuf::from("/tmp/repo"),
        needs_setup: true,
    };

    assert!(!repository_setup_needed(Some(&repo), Scope::Global));
}

#[test]
fn repository_targets_need_repository_setup_when_repo_is_not_setup() {
    let repo = RepoInfo {
        root: PathBuf::from("/tmp/repo"),
        needs_setup: true,
    };

    assert!(repository_setup_needed(Some(&repo), Scope::Repository));
    assert!(repository_setup_needed(Some(&repo), Scope::Both));
}

#[test]
fn repository_targets_do_not_need_setup_when_repo_is_already_setup() {
    let repo = RepoInfo {
        root: PathBuf::from("/tmp/repo"),
        needs_setup: false,
    };

    assert!(!repository_setup_needed(Some(&repo), Scope::Repository));
}

//! `but skill` printing the embedded guide, and the hidden `--stub` install
//! layout that points agents at it.

use snapbox::str;

use super::{corrupt_repository, relative_agent_skill_path};
use crate::utils::{CommandExt, Sandbox};

#[test]
fn skill_prints_the_core_guide_without_frontmatter() {
    let env = Sandbox::empty();

    // The frontmatter is trigger text for the harness, not guidance; the body
    // starts at the title and ends on the pointers to the other docs.
    env.but("skill").assert().success().stdout_eq(str![[r#"
# GitButler CLI Skill

`but` is the version-control interface. GitButler keeps several branches applied in one working directory and assigns changes to them, so anything that moves refs, the index or the working tree (`add`, `commit`, `checkout`, `reset`, `restore`, `rebase`, `merge`, `stash`, `cherry-pick`, `fetch`, `pull`, `push`) goes through `but`; run raw they bypass its bookkeeping, and the workspace they leave behind is not the one `but` describes. Read-only git (`log`, `blame`, `show`) is fine. When the user names a git write command, run the `but` equivalent.
...
## More

- `but skill reference`: every command with its arguments and flags.
- `but skill concepts`: the workspace model, applied and unapplied branches, IDs in depth.
- `but <cmd> --help`: examples for one command.

"#]]);
}

#[test]
fn skill_prints_the_guide_inside_an_unreadable_repository() {
    let env = Sandbox::empty();
    corrupt_repository(&env);

    env.but("skill").assert().success().stdout_eq(str![[r#"
# GitButler CLI Skill
...
"#]]);
}

#[test]
fn skill_full_appends_every_reference_with_separators() {
    let env = Sandbox::empty();

    env.but("skill --full")
        .assert()
        .success()
        .stdout_eq(str![[r#"
# GitButler CLI Skill
...
--- but skill reference ---

# but command reference
...
--- but skill concepts ---
...
--- but skill examples ---
...
"#]]);
}

#[test]
fn skill_doc_subcommands_print_one_reference_each() {
    let env = Sandbox::empty();

    env.but("skill concepts")
        .assert()
        .success()
        .stdout_eq(str![[r#"
# GitButler CLI Key Concepts
...
"#]]);
    // `--full` only makes sense for the core guide.
    env.but("skill reference --full")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
error: unexpected argument '--full' found

Usage: but skill reference [OPTIONS]

For more information, try '--help'.

"#]]);
}

/// The reference is rendered from the clap tree, so a wording change to any
/// command's help shows up here as a reviewable diff. The tree, and so the
/// snapshot, is the legacy command set.
#[test]
#[cfg(feature = "legacy")]
fn skill_reference_is_rendered_from_the_command_tree() {
    let env = Sandbox::empty();

    env.but("skill reference")
        .assert()
        .success()
        .stdout_eq(snapbox::file!["snapshots/reference.md"]);
}

#[test]
fn skill_json_carries_the_doc_and_full_references() {
    let env = Sandbox::empty();

    let output = env
        .but("skill --full --json")
        .allow_json()
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["name"], "core");
    assert!(
        json["content"]
            .as_str()
            .is_some_and(|content| content.starts_with("# GitButler CLI Skill")),
        "content is the frontmatter-free body"
    );
    let names: Vec<&str> = json["references"]
        .as_array()
        .expect("--full lists the reference documents")
        .iter()
        .map(|doc| doc["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["reference", "concepts", "examples"]);
}

/// The guide `but skill` prints is a separate document from the SKILL.md that
/// installation writes, so the served wording can change without altering
/// what agents have on disk.
#[test]
fn skill_serves_a_different_guide_than_install_writes() {
    let env = Sandbox::open_with_default_settings("repo-no-remote");
    let install_path = relative_agent_skill_path(".agents");

    env.but("")
        .arg("skill")
        .arg("install")
        .arg("--path")
        .arg(&install_path)
        .assert()
        .success();

    let install_dir = env.projects_root().join(&install_path);
    let installed = std::fs::read_to_string(install_dir.join("SKILL.md")).unwrap();
    assert!(
        installed.contains("Use GitButler CLI (`but`) as the default version-control interface."),
        "install writes the embedded SKILL.md, got: {installed}"
    );

    let served = env
        .but("skill")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let served = String::from_utf8(served).unwrap();
    assert!(
        served.starts_with("# GitButler CLI Skill\n\n`but` is the version-control interface."),
        "but skill prints the served guide, got: {served}"
    );
    assert!(
        !installed.contains("`but` is the version-control interface."),
        "the served guide is not what gets installed"
    );
}

#[test]
fn skill_install_stub_writes_one_marked_file_that_check_accepts() {
    let env = Sandbox::open_with_default_settings("repo-no-remote");
    let install_path = relative_agent_skill_path(".agents");

    env.but("")
        .arg("skill")
        .arg("install")
        .arg("--stub")
        .arg("--path")
        .arg(&install_path)
        .assert()
        .success()
        .stdout_eq(str![[r#"

✓ GitButler skill installed successfully!

  Location: ./.agents/skills/gitbutler

  Files installed:
    • SKILL.md


"#]]);

    let install_dir = env.projects_root().join(&install_path);
    let skill_md = std::fs::read_to_string(install_dir.join("SKILL.md")).unwrap();
    assert!(
        skill_md.starts_with("---\nname: but\n"),
        "the stub keeps the full skill's frontmatter so triggering is unchanged"
    );
    assert!(
        skill_md.contains("\nstub: true\nallowed-tools: Bash(but skill:*)\n---\n"),
        "the stub is marked and pre-approves the read it asks for, got: {skill_md}"
    );
    assert!(
        skill_md.contains("\n```bash\nbut skill\n```\n"),
        "the stub body points at the CLI, got: {skill_md}"
    );
    assert!(
        !install_dir.join("references").exists(),
        "a stub carries no reference files"
    );

    // A stub is a complete installation, not an incomplete bundle.
    let output = env
        .but("skill check --local --json")
        .allow_json()
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(json["outdated_count"], 0);
}

#[test]
fn skill_install_stub_over_a_full_install_removes_the_references() {
    let env = Sandbox::open_with_default_settings("repo-no-remote");
    let install_path = relative_agent_skill_path(".agents");
    env.but("")
        .arg("skill")
        .arg("install")
        .arg("--path")
        .arg(&install_path)
        .assert()
        .success();
    let install_dir = env.projects_root().join(&install_path);
    assert!(install_dir.join("references").is_dir());

    env.but("")
        .arg("skill")
        .arg("install")
        .arg("--stub")
        .arg("--path")
        .arg(&install_path)
        .assert()
        .success();

    assert!(
        !install_dir.join("references").exists(),
        "a full install converted to a stub leaves no stale reference files"
    );
    assert!(
        std::fs::read_to_string(install_dir.join("SKILL.md"))
            .unwrap()
            .contains("\nstub: true\n")
    );
}

#[test]
fn skill_install_stub_removes_a_stray_references_file() {
    let env = Sandbox::open_with_default_settings("repo-no-remote");
    let install_path = relative_agent_skill_path(".agents");
    env.but("")
        .args(["skill", "install", "--path"])
        .arg(&install_path)
        .assert()
        .success();
    let install_dir = env.projects_root().join(&install_path);
    std::fs::remove_dir_all(install_dir.join("references")).unwrap();
    std::fs::write(install_dir.join("references"), "not a directory").unwrap();

    env.but("")
        .args(["skill", "install", "--stub", "--path"])
        .arg(&install_path)
        .assert()
        .success();

    assert!(
        !install_dir.join("references").exists(),
        "any stale references entry is removed, not only a directory"
    );
}

#[cfg(unix)]
#[test]
fn skill_install_stub_removes_a_symlinked_references_dir_without_following_it() {
    let env = Sandbox::open_with_default_settings("repo-no-remote");
    let install_path = relative_agent_skill_path(".agents");
    env.but("")
        .args(["skill", "install", "--path"])
        .arg(&install_path)
        .assert()
        .success();
    let install_dir = env.projects_root().join(&install_path);
    let target = env.projects_root().join("elsewhere");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(target.join("keep.md"), "canary").unwrap();
    std::fs::remove_dir_all(install_dir.join("references")).unwrap();
    std::os::unix::fs::symlink(&target, install_dir.join("references")).unwrap();

    env.but("")
        .args(["skill", "install", "--stub", "--path"])
        .arg(&install_path)
        .assert()
        .success();

    assert!(
        !install_dir.join("references").exists(),
        "the link itself is removed"
    );
    assert!(
        target.join("keep.md").is_file(),
        "the link target is never followed"
    );
}

#[test]
fn skill_install_detect_reports_files_per_location_when_layouts_differ() {
    let env = Sandbox::open_with_default_settings("repo-no-remote");
    let full = relative_agent_skill_path(".agents");
    let stub = relative_agent_skill_path(".claude");
    env.but("")
        .args(["skill", "install", "--path"])
        .arg(&full)
        .assert()
        .success();
    env.but("")
        .args(["skill", "install", "--stub", "--path"])
        .arg(&stub)
        .assert()
        .success();

    // `--detect` refreshes both and keeps each layout; the report follows suit.
    let output = env
        .but("skill install --detect --json")
        .allow_json()
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let mut by_path: Vec<(String, usize)> = json["installations"]
        .as_array()
        .expect("installations are listed per path")
        .iter()
        .map(|entry| {
            (
                entry["path"].as_str().unwrap().replace('\\', "/"),
                entry["files"].as_array().unwrap().len(),
            )
        })
        .collect();
    by_path.sort();
    assert_eq!(by_path.len(), 2);
    assert!(
        by_path[0].0.ends_with(".agents/skills/gitbutler") && by_path[0].1 == 4,
        "the full install keeps all four files, got: {by_path:?}"
    );
    assert!(
        by_path[1].0.ends_with(".claude/skills/gitbutler") && by_path[1].1 == 1,
        "the stub install keeps one file, got: {by_path:?}"
    );
    assert!(
        json.get("files").is_none(),
        "no single file list describes mixed layouts"
    );

    env.but("skill install --detect")
        .assert()
        .success()
        .stdout_eq(str![[r#"
...
  Locations and files installed:
    • [..].agents/skills/gitbutler
        references/reference.md
        references/concepts.md
        references/examples.md
        SKILL.md
    • [..].claude/skills/gitbutler
        SKILL.md


"#]]);
}

#[test]
fn skill_check_update_keeps_a_stale_stub_a_stub() {
    let env = Sandbox::open_with_default_settings("repo-no-remote");
    let install_path = relative_agent_skill_path(".agents");
    env.but("")
        .arg("skill")
        .arg("install")
        .arg("--stub")
        .arg("--path")
        .arg(&install_path)
        .assert()
        .success();
    let skill_md_path = env.projects_root().join(&install_path).join("SKILL.md");
    let stale = std::fs::read_to_string(&skill_md_path)
        .unwrap()
        .replace("version: dev", "version: 0.0.1");
    std::fs::write(&skill_md_path, stale).unwrap();

    env.but("skill check --local --update").assert().success();

    let refreshed = std::fs::read_to_string(&skill_md_path).unwrap();
    assert!(
        refreshed.contains("version: dev") && refreshed.contains("\nstub: true\n"),
        "an outdated stub is rewritten as a current stub, never upgraded to the full bundle"
    );
    assert!(
        !env.projects_root()
            .join(&install_path)
            .join("references")
            .exists()
    );
}

#[test]
fn agent_skill_notice_accepts_a_stub_install() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);
    env.but("skill install --stub --path .agents/skills/gitbutler")
        .assert()
        .success();

    let output = env
        .but("alias list")
        .env("AI_AGENT", "opencode")
        .output()
        .expect("alias list runs");
    assert!(output.status.success());
    assert!(
        !String::from_utf8_lossy(&output.stdout).contains("AGENT ACTION REQUIRED"),
        "a stub counts as an installed, current skill"
    );
}

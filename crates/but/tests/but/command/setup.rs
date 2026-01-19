use crate::utils::Sandbox;

#[test]
fn not_a_git_repository() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    env.but("setup")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Failed to set up GitButler project.

Caused by:
    No git repository found - run `but setup` to initialize a new repository.

"#]]);

    Ok(())
}

#[test]
fn no_remote_creates_gb_local() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote")?;

    // Verify initial state - no remotes
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("remote")
        .output()?;
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "");

    // Run setup
    env.but("setup")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Setting up GitButler project...

→ Adding repository to GitButler project registry
  ✓ Repository added to project registry

→ Configuring default target branch
  No push remote found, creating gb-local remote...
  ✓ Created gb-local remote tracking main
  ✓ Set default target to: gb-local/main

GitButler project setup complete!

Repository: [..]
Default target: gb-local/main
Remote: gb-local


"#]]);

    // Verify gb-local remote was created
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("remote")
        .output()?;
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "gb-local");

    // Verify remote HEAD was created
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("symbolic-ref")
        .arg("refs/remotes/gb-local/HEAD")
        .output()?;
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "refs/remotes/gb-local/main"
    );

    Ok(())
}

#[test]
fn no_remote_with_non_standard_branch() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-no-remote-no-main-or-master")?;

    // Run setup - should use the current branch name (development)
    env.but("setup")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Setting up GitButler project...

→ Adding repository to GitButler project registry
  ✓ Repository added to project registry

→ Configuring default target branch
  No push remote found, creating gb-local remote...
  ✓ Created gb-local remote tracking development
  ✓ Set default target to: gb-local/development

GitButler project setup complete!

Repository: [..]
Default target: gb-local/development
Remote: gb-local


"#]]);

    // Verify gb-local remote was created with development branch
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("symbolic-ref")
        .arg("refs/remotes/gb-local/HEAD")
        .output()?;
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "refs/remotes/gb-local/development"
    );

    Ok(())
}

#[test]
fn remote_exists_but_no_remote_head() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-with-remote-no-head")?;

    // Verify remote exists but no HEAD
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("remote")
        .output()?;
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "origin");

    // Verify no remote HEAD exists initially
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("symbolic-ref")
        .arg("refs/remotes/origin/HEAD")
        .output()?;
    assert!(!output.status.success());

    // Run setup - should fail because there's no remote HEAD to discover
    env.but("setup")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Setting up GitButler project...

→ Adding repository to GitButler project registry
  ✓ Repository added to project registry

→ Configuring default target branch
  ✓ Using existing push remote: origin
  ✓ No remote HEAD found, using origin/main
  ✓ Set default target to: origin/main

GitButler project setup complete!

Repository: .
Default target: origin/main
Remote: origin


"#]]);

    Ok(())
}

#[test]
fn remote_exists_with_head() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-with-remote-and-head")?;

    // Verify remote exists with HEAD
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("remote")
        .output()?;
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "origin");

    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(env.projects_root())
        .arg("symbolic-ref")
        .arg("refs/remotes/origin/HEAD")
        .output()?;
    assert!(output.status.success());

    // Run setup
    env.but("setup")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Setting up GitButler project...

→ Adding repository to GitButler project registry
  ✓ Repository added to project registry

→ Configuring default target branch
  ✓ Using existing push remote: origin
  ✓ Set default target to: origin/main

GitButler project setup complete!

Repository: [..]
Default target: origin/main
Remote: origin


"#]]);

    Ok(())
}

#[test]
fn already_setup() -> anyhow::Result<()> {
    let env = Sandbox::open_with_default_settings("repo-already-setup")?;

    // Run setup once to initialize
    env.but("setup").assert().success();

    // Run setup again - should recognize it's already set up
    // Note: The project gets re-added because Sandbox.empty() creates fresh temp dirs each time,
    // but the target is detected as already configured
    env.but("setup")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
Setting up GitButler project...

→ Adding repository to GitButler project registry
  ✓ Repository added to project registry

GitButler project is already set up!

Repository: .
Default target: origin/main


"#]]);

    Ok(())
}

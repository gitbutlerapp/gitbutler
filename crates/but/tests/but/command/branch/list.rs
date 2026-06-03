use crate::utils::{CommandExt, Sandbox};

/// Hide empty applied branches by default and show them again with `--empty`.
#[test]
fn list_hides_empty_applied_branches_by_default() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks-one-empty")?;
    env.setup_metadata(&["A", "B"])?;

    let result = env.but("--format json branch list").allow_json().output()?;
    assert!(result.status.success());
    let stdout = String::from_utf8_lossy(&result.stdout);
    let json: serde_json::Value = serde_json::from_str(stdout.trim())?;

    let applied_heads: Vec<_> = json["appliedStacks"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|stack| stack["heads"].as_array().unwrap())
        .map(|head| head["name"].as_str().unwrap())
        .collect();

    assert!(applied_heads.contains(&"A"));
    assert!(!applied_heads.contains(&"B"));

    assert!(json["branches"].as_array().unwrap().is_empty());

    let result = env
        .but("--format json branch list --empty")
        .allow_json()
        .output()?;
    assert!(result.status.success());
    let stdout = String::from_utf8_lossy(&result.stdout);
    let json: serde_json::Value = serde_json::from_str(stdout.trim())?;

    let applied_heads: Vec<_> = json["appliedStacks"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|stack| stack["heads"].as_array().unwrap())
        .map(|head| head["name"].as_str().unwrap())
        .collect();

    assert!(applied_heads.contains(&"A"));
    assert!(applied_heads.contains(&"B"));

    Ok(())
}

#[test]
fn list_truncates_results_if_they_exceed_default_limit() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    env.setup_metadata(&["A"])?;
    let default_limit = 20;

    for i in 0..default_limit {
        env.invoke_git(&format!("branch branch-{i} A"));
    }

    env.but("branch list")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Applied branches
active  ✓ *A          26y ago    author

Unapplied Branches
local   ✓ branch-0  ↑1     26y ago    author
local   ✓ branch-1  ↑1     26y ago    author
local   ✓ branch-10 ↑1     26y ago    author
local   ✓ branch-11 ↑1     26y ago    author
local   ✓ branch-12 ↑1     26y ago    author
local   ✓ branch-13 ↑1     26y ago    author
local   ✓ branch-14 ↑1     26y ago    author
local   ✓ branch-15 ↑1     26y ago    author
local   ✓ branch-16 ↑1     26y ago    author
local   ✓ branch-17 ↑1     26y ago    author
local   ✓ branch-18 ↑1     26y ago    author
local   ✓ branch-19 ↑1     26y ago    author
local   ✓ branch-2  ↑1     26y ago    author
local   ✓ branch-3  ↑1     26y ago    author
local   ✓ branch-4  ↑1     26y ago    author
local   ✓ branch-5  ↑1     26y ago    author
local   ✓ branch-6  ↑1     26y ago    author
local   ✓ branch-7  ↑1     26y ago    author
local   ✓ branch-8  ↑1     26y ago    author
local   ✓ branch-9  ↑1     26y ago    author

"#]])
        .stderr_eq(snapbox::str![[]]);

    // This should be the tipping point for truncating branch results
    env.invoke_git("branch branch-20 A");

    env.but("branch list")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Applied branches
active  ✓ *A          26y ago    author

Unapplied Branches
local   ✓ branch-0  ↑1     26y ago    author
local   ✓ branch-1  ↑1     26y ago    author
local   ✓ branch-10 ↑1     26y ago    author
local   ✓ branch-11 ↑1     26y ago    author
local   ✓ branch-12 ↑1     26y ago    author
local   ✓ branch-13 ↑1     26y ago    author
local   ✓ branch-14 ↑1     26y ago    author
local   ✓ branch-15 ↑1     26y ago    author
local   ✓ branch-16 ↑1     26y ago    author
local   ✓ branch-17 ↑1     26y ago    author
local   ✓ branch-18 ↑1     26y ago    author
local   ✓ branch-19 ↑1     26y ago    author
local   ✓ branch-2  ↑1     26y ago    author
local   ✓ branch-20 ↑1     26y ago    author
local   ✓ branch-3  ↑1     26y ago    author
local   ✓ branch-4  ↑1     26y ago    author
local   ✓ branch-5  ↑1     26y ago    author
local   ✓ branch-6  ↑1     26y ago    author
local   ✓ branch-7  ↑1     26y ago    author
local   ✓ branch-8  ↑1     26y ago    author

... result truncated to 20 matching branches (use --all to show all that match filters)

"#]])
        .stderr_eq(snapbox::str![[]]);

    Ok(())
}

use crate::utils::{CommandExt, Sandbox};

#[test]
fn uncommitted() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file(
        "file",
        r#"
items = ["ink ribbon", "old key", "green herb", "crank", "lighter"]

puts "You check the desk drawer..."
sleep 0.8

found = items.sample

if found == "green herb"
  puts "You found a #{found}."
  puts "You feel just a little better."
else
  puts "You found an #{found}." rescue puts "You found a #{found}."
  puts "Probably useful somewhere."
end

puts "\nA distant door unlocks."
"#,
    );

    env.but("_diff2")
        .with_color_for_svg()
        .assert()
        .success()
        .stdout_eq(snapbox::file![
            "snapshots/diff2/uncommitted.stdout.term.svg"
        ]);

    env.but("_diff2")
        .with_color_for_svg()
        .assert()
        .success()
        .stdout_eq(snapbox::file!["snapshots/diff2/uncommitted.stdout"].raw());
}

#[test]
fn path_prefix() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    env.file("a/b/c.txt", "content of c");
    env.file("a/b/d.txt", "content of d");
    env.file("a/b.txt", "content of b");

    env.but("_diff2 a/b/")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
────────────────╮
 up:2 a/b/c.txt │
────────────────╯
 
@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +content of c

────────────────╮
 oz:8 a/b/d.txt │
────────────────╯
 
@@ -1,0 +1,1 @@
───────────────
  ┊ 1 │ +content of d

"#]]);
}

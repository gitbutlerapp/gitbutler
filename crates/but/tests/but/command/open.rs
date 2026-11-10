use but_api::open::program::USER_DEFINED_PROGRAMS_FILENAME;

use crate::{command::util::add_dirty_worktree, utils::Sandbox};

fn setup_multi_hunk_uncommitted_changes(path: &str) -> Sandbox {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    let original_content = "this\nis\nsome\ncontent\nto\ndiff\nwith\nadded\nlines\n";
    env.file(path, original_content);
    env.but("commit -m 'Add file'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Created commit [..] on new branch 'a-branch-1'

"#]]);

    env.file(path, format!("new first\n{original_content}new last"));

    env
}

fn setup_multi_file_uncommitted_changes(paths: &[&str]) -> Sandbox {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);

    for path in paths {
        env.file(path, "content");
    }

    env
}

#[test]
fn open_with_ambiguous_program() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.file("new-file.txt", "content");

    env.but("_open new-file.txt")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Could not automatically choose program. Found nvim, cursor, sublime, vscode, zed, echo, touch

Hint: Specify a program with `--program-id`

"#]]);
}

#[test]
fn open_uncommitted_file_with() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");

    env.file("new-file.txt", "content");

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted]
┊   xk A new-file.txt
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

Hint: run `but branch new` to create a new branch to work on

"#]]);

    env.but("_open xk -p echo")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
filepath='/[..]/new-file.txt'

"#]]);
}

#[test]
fn open_multiple_uncommitted_files() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");

    env.file("first.txt", "first");
    env.file("second.txt", "second");

    env.but("_open first.txt second.txt -p echo")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
filepath='/[..]/first.txt' filepath='/[..]/second.txt'

"#]]);
}

#[test]
fn open_ignores_metadata_for_missing_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&["A"]);
    env.file("uncommitted.txt", "content");

    env.but("_open uncommitted.txt -p echo")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
filepath='/[..]/uncommitted.txt'

"#]]);

    assert!(
        env.open_repo()
            .try_find_reference("refs/heads/A")
            .expect("reference lookup succeeds")
            .is_none(),
        "opening a file must not recreate a branch from stale metadata"
    );
}

#[test]
fn open_uncommitted_hunk() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");

    let original_content = "this\nis\nsome\ncontent\nto\ndiff\nwith\nadded\nlines\n";
    env.file("file-with-additions.txt", original_content);
    env.file("file-with-deletions.txt", original_content);
    env.file("file-with-mixed.txt", original_content);
    env.but("commit -m 'Add files'").assert().success();

    env.file(
        "file-with-additions.txt",
        format!("new first\n{original_content}new last"),
    );
    env.file(
        "file-with-deletions.txt",
        "is\nsome\ncontent\nto\ndiff\nwith\nadded\n",
    );
    env.file(
        "file-with-mixed.txt",
        "this\nIS\nsome\ncontent\nto\ndiff\nwith\nADDED\n",
    );

    env.but("diff")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
────────────────────────────────╮
 rn:7 M file-with-additions.txt │
────────────────────────────────╯

@@ -1,3 +1,4 @@
───────────────
  ┊ 1 │ +new first
1 ┊ 2 │  this
2 ┊ 3 │  is
3 ┊ 4 │  some

────────────────────────────────╮
 rn:4 M file-with-additions.txt │
────────────────────────────────╯

@@ -7,3 +8,4 @@
───────────────
 7 ┊  8 │  with
 8 ┊  9 │  added
 9 ┊ 10 │  lines
   ┊ 11 │ +new last

────────────────────────────────╮
 rw:b M file-with-deletions.txt │
────────────────────────────────╯

@@ -1,4 +1,3 @@
───────────────
1 ┊   │ -this
2 ┊ 1 │  is
3 ┊ 2 │  some
4 ┊ 3 │  content

────────────────────────────────╮
 rw:6 M file-with-deletions.txt │
────────────────────────────────╯

@@ -6,4 +5,3 @@
───────────────
 6 ┊ 5 │  diff
 7 ┊ 6 │  with
 8 ┊ 7 │  added
 9 ┊   │ -lines

────────────────────────────╮
 lp:6 M file-with-mixed.txt │
────────────────────────────╯

@@ -1,9 +1,8 @@
───────────────
 1 ┊ 1 │  this
 2 ┊   │ -is
   ┊ 2 │ +IS
 3 ┊ 3 │  some
 4 ┊ 4 │  content
 5 ┊ 5 │  to
 6 ┊ 6 │  diff
 7 ┊ 7 │  with
 8 ┊   │ -added
 9 ┊   │ -lines
   ┊ 8 │ +ADDED

"#]]);

    env.but("_open rn:7 -p echo ")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
filepath='/[..]/file-with-additions.txt' line_number='1'

"#]]);
    env.but("_open rn:4 -p echo ")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
filepath='/[..]/file-with-additions.txt' line_number='11'

"#]]);
    env.but("_open rw:b -p echo ")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
filepath='/[..]/file-with-deletions.txt' line_number='1'

"#]]);
    env.but("_open rw:6 -p echo ")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
filepath='/[..]/file-with-deletions.txt' line_number='7'

"#]]);
    env.but("_open lp:6 -p echo ")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
filepath='/[..]/file-with-mixed.txt' line_number='2'

"#]]);
}

#[test]
fn cannot_open_hunk_with_multiple_sources() {
    let env = setup_multi_hunk_uncommitted_changes("file.txt");

    env.but("_open file.txt uv:4 -p echo")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Only entire files can be opened when multiple sources are provided; 'uv:4' is a hunk

"#]]);
}

#[test]
fn open_uncommitted_hunk_in_file_that_contains_spaces_and_shell_metacharacters() {
    let env = setup_multi_hunk_uncommitted_changes(
        "file with some $meta; cat A > new-file.txt; spaces in it.txt",
    );

    env.but("status").assert().success().stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted]
┊   pv M file with some $meta; cat A > new-file.txt; spaces in it.txt
┊
┊╭┄ br [a-branch-1]
┊●   psz Add file
├╯
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    env.but("_open pv:4 -p echo")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
filepath='/[..]/file with some $meta; cat A > new-file.txt; spaces in it.txt' line_number='11'

"#]]);
}

#[test]
fn cannot_open_non_existing_cli_id() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");

    env.but("_open notexist -p echo")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Could not find target: 'notexist'

Hint: Run `but status` for applicable targets.

"#]]);
}

#[test]
fn open_committed_file() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("_open tpm:t -p echo")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
filepath='/[..]/A'

"#]]);
}

#[test]
fn open_committed_hunks() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    let content = "one\ntwo\nthree\nfour\nfive\nsix\nseven\n";

    env.file("file", content);
    env.but("commit -m 'Add file'").assert().success();
    env.file("file", format!("beginning\n{content}end"));
    env.but("commit -m 'Update file'").assert().success();

    env.but("_open szk:q:3 -p echo")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
filepath='/[..]/file' line_number='1'

"#]]);
    env.but("_open szk:q:8 -p echo")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
filepath='/[..]/file' line_number='9'

"#]]);
}

#[test]
fn cannot_open_branch_or_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("_open A -p echo")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Expected file or hunk, got a branch

"#]]);

    env.but("_open tpm -p echo")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Expected file or hunk, got a commit

"#]]);
}

#[test]
fn cannot_open_with_unknown_program() {
    let env = setup_multi_hunk_uncommitted_changes("file.txt");
    env.but("_open file.txt -p nosuchprogram")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Bad input 'nosuchprogram' for '--program-id'

No such program found

"#]]);
}

#[test]
fn user_defined_program_path_executable_handles_shell_metacharacters() {
    let env = setup_multi_hunk_uncommitted_changes(
        "file with some $meta; cat A > new-file.txt; spaces in it.txt",
    );

    let programs_json = env
        .app_data_dir()
        .join("gitbutler")
        .join(USER_DEFINED_PROGRAMS_FILENAME);

    std::fs::write(
        programs_json,
        r#"[
   {
     "id": "test-program",
     "name": "Test Program",
     "executable": {
       "type": "pathExecutable",
       "nameOrPath": "echo",
       "requiresTerminal": true
     },
     "category": "other",
     "openArgs": [
       "Test Program - Open File:",
       "filepath='{{filepath}}'"
     ],
     "openAtLineArgs": [
       "Test Program - Open File At Line:",
       "line_number='{{line_number}}'",
       "filepath='{{filepath}}'"
     ]
   }
]"#,
    )
    .unwrap();

    env.but("status -f").assert().success().stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted]
┊   pv M file with some $meta; cat A > new-file.txt; spaces in it.txt
┊
┊╭┄ br [a-branch-1]
┊●   psz Add file
┊│     psz:p A file with some $meta; cat A > new-file.txt; spaces in it.txt
├╯
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    env.but("_open pv -p test-program").assert().success().stdout_eq(snapbox::str![[r#"
Test Program - Open File: filepath='/[..]/file with some $meta; cat A > new-file.txt; spaces in it.txt'

"#]]);

    env.but("diff")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
─────────────────────────────────────────────────────────────────────╮
 pv:7 M file with some $meta; cat A > new-file.txt; spaces in it.txt │
─────────────────────────────────────────────────────────────────────╯

@@ -1,3 +1,4 @@
───────────────
  ┊ 1 │ +new first
1 ┊ 2 │  this
2 ┊ 3 │  is
3 ┊ 4 │  some

─────────────────────────────────────────────────────────────────────╮
 pv:4 M file with some $meta; cat A > new-file.txt; spaces in it.txt │
─────────────────────────────────────────────────────────────────────╯

@@ -7,3 +8,4 @@
───────────────
 7 ┊  8 │  with
 8 ┊  9 │  added
 9 ┊ 10 │  lines
   ┊ 11 │ +new last

"#]]);

    env.but("_open pv:4 -p test-program ").assert().success().stdout_eq(snapbox::str![[r#"
Test Program - Open File At Line: line_number='11' filepath='/[..]/file with some $meta; cat A > new-file.txt; spaces in it.txt'

"#]]);
}

/// For most programs, you get something usable by just defining the executable and passing the file
/// as the first argument.
#[test]
fn user_defined_program_defaults_to_default_open_args() {
    let env = setup_multi_hunk_uncommitted_changes("file.txt");

    let programs_json = env
        .app_data_dir()
        .join("gitbutler")
        .join(USER_DEFINED_PROGRAMS_FILENAME);

    std::fs::write(
        programs_json,
        r#"[
   {
     "id": "test-program-no-args",
     "name": "Test Program No Args",
     "executable": {
       "type": "pathExecutable",
       "nameOrPath": "echo",
       "requiresTerminal": true
     },
     "category": "other"
   },
   {
     "id": "test-program-only-open-args",
     "name": "Test Program Only Open Args",
     "executable": {
       "type": "pathExecutable",
       "nameOrPath": "echo",
       "requiresTerminal": true
     },
     "category": "other",
     "openArgs": [
       "filepath='{{filepath}}'"
     ]
   },
   {
     "id": "test-program-only-open-at-args",
     "name": "Test Program Only Open At Args",
     "executable": {
       "type": "pathExecutable",
       "nameOrPath": "echo",
       "requiresTerminal": true
     },
     "category": "other",
     "openAtLineArgs": [
       "line_number='{{line_number}}'",
       "filepath='{{filepath}}'"
     ]
   }
]"#,
    )
    .unwrap();

    env.but("status -f").assert().success().stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted]
┊   uv M file.txt
┊
┊╭┄ br [a-branch-1]
┊●   zon Add file
┊│     zon:u A file.txt
├╯
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    env.but("diff")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
─────────────────╮
 uv:7 M file.txt │
─────────────────╯

@@ -1,3 +1,4 @@
───────────────
  ┊ 1 │ +new first
1 ┊ 2 │  this
2 ┊ 3 │  is
3 ┊ 4 │  some

─────────────────╮
 uv:4 M file.txt │
─────────────────╯

@@ -7,3 +8,4 @@
───────────────
 7 ┊  8 │  with
 8 ┊  9 │  added
 9 ┊ 10 │  lines
   ┊ 11 │ +new last

"#]]);

    // No-args program, should always only get the filepath passed by the default CLI arg supplier
    env.but("_open uv -p test-program-no-args")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
/[..]/file.txt

"#]]);
    env.but("_open uv:4 -p test-program-no-args")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
/[..]/file.txt

"#]]);

    // Open args defined, should get the custom open args both for open and open at line
    env.but("_open uv -p test-program-only-open-args")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
filepath='/[..]/file.txt'

"#]]);
    env.but("_open uv:4 -p test-program-only-open-args")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
filepath='/[..]/file.txt'

"#]]);

    // Open at line args defined, should get default open and custom open at line
    env.but("_open uv -p test-program-only-open-at-args")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
/[..]/file.txt

"#]]);
    env.but("_open uv:4 -p test-program-only-open-at-args")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
line_number='11' filepath='/[..]/file.txt'

"#]]);
}

#[test]
fn ignores_malformed_user_defined_programs_file() {
    let env = setup_multi_hunk_uncommitted_changes("file.txt");

    let programs_json = env
        .app_data_dir()
        .join("gitbutler")
        .join(USER_DEFINED_PROGRAMS_FILENAME);

    std::fs::write(
        programs_json,
        r#"[
   {
     "id": "test-program",
     "name": "Test Program",
     "executable": {
       "type": "pathExecutable",
       "nameOrPath": "echo",
   "#,
    )
    .unwrap();

    env.but("_open file.txt -p test-program")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Bad input 'test-program' for '--program-id'

No such program found

"#]]);

    // Can still successfully use built-ins
    env.but("_open file.txt -p echo")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
filepath='/[..]/file.txt'

"#]]);
}

#[test]
fn user_defined_program_derives_id_from_name_if_id_is_omitted() {
    let env = setup_multi_hunk_uncommitted_changes("file.txt");

    let programs_json = env
        .app_data_dir()
        .join("gitbutler")
        .join(USER_DEFINED_PROGRAMS_FILENAME);

    std::fs::write(
        programs_json,
        r#"[
   {
     "name": "Test Program",
     "executable": {
       "type": "pathExecutable",
       "nameOrPath": "echo",
       "requiresTerminal": true
     },
     "category": "other",
     "openArgs": [
       "Test Program - Open File:",
       "filepath='{{filepath}}'"
     ],
     "openAtLineArgs": [
       "Test Program - Open File At Line:",
       "line_number='{{line_number}}'",
       "filepath='{{filepath}}'"
     ]
   }
]
   "#,
    )
    .unwrap();

    env.but("_open file.txt -p 'Test Program'")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Test Program - Open File: filepath='/[..]/file.txt'

"#]]);

    // Can still successfully use built-ins
    env.but("_open file.txt -p echo")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
filepath='/[..]/file.txt'

"#]]);
}

#[test]
fn user_defined_programs_can_override_builtins() {
    let env = setup_multi_hunk_uncommitted_changes("file.txt");

    let programs_json = env
        .app_data_dir()
        .join("gitbutler")
        .join(USER_DEFINED_PROGRAMS_FILENAME);

    std::fs::write(
        programs_json,
        r#"[
   {
     "id": "echo",
     "openArgs": [
       "CUSTOM OVERRIDE='{{filepath}}'"
     ]
   }
]
   "#,
    )
    .unwrap();

    env.but("_open file.txt -p echo")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
CUSTOM OVERRIDE='/[..]/file.txt'

"#]]);
}

#[test]
fn user_defined_programs_list_can_set_extension_list_for_builtins() {
    let env = setup_multi_file_uncommitted_changes(&["file.txt", "file.md", "Dockerfile"]);

    let programs_json = env
        .app_data_dir()
        .join("gitbutler")
        .join(USER_DEFINED_PROGRAMS_FILENAME);

    std::fs::write(
        programs_json,
        serde_json::to_string_pretty(&serde_json::json!(
            [
                {
                  "name": "Sentinel to make sure we don't just pick the first definition",
                  "executable": {
                    "type": "pathExecutable",
                    "nameOrPath": "echo",
                    "requiresTerminal": true
                  },
                  "openArgs": [
                    "WRONG PROGRAM"
                  ]
                },
                {
                  "id": "echo",
                  "extensions": ["txt"]
                },
                {
                  "id": "touch",
                  "extensions": ["*"]
                }
            ]
        ))
        .unwrap(),
    )
    .unwrap();

    // should find echo by extension and touch by wildcard
    env.but("_open file.txt")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Could not automatically choose program. Found echo, touch

Hint: Specify a program with `--program-id`

"#]]);

    // should find echo by extension and touch by wildcard
    env.but("_open file.txt --program-id echo")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
filepath='/[..]/file.txt'

"#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted]
┊   tt A Dockerfile
┊   zn A file.md
┊   uv A file.txt
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

Hint: run `but branch new` to create a new branch to work on

"#]]);

    // should find touch by wildcard
    env.but("_open file.md")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#""#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted]
┊   tt A Dockerfile
┊   zn A file.md
┊   ul A file.md.touch
┊   uv A file.txt
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

Hint: run `but branch new` to create a new branch to work on

"#]]);

    // should find touch by wildcard
    env.but("_open Dockerfile")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#""#]]);

    env.but("status")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted]
┊   tt A Dockerfile
┊   mu A Dockerfile.touch
┊   zn A file.md
┊   ul A file.md.touch
┊   uv A file.txt
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

Hint: run `but branch new` to create a new branch to work on

"#]]);

    // should still be able to explicitly select a different handler
    env.but("_open Dockerfile -p echo")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
filepath='/[..]/Dockerfile'

"#]]);

    // should find echo by extension and touch by wildcard
    env.but("_open file.txt file.md Dockerfile")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Could not automatically choose program. Found echo, touch

Hint: Specify a program with `--program-id`

"#]]);

    // should select echo as it's handler for the first file
    env.but("_open file.txt file.md Dockerfile --program-id echo")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
filepath='/[..]/file.txt' filepath='/[..]/file.md' filepath='/[..]/Dockerfile'

"#]]);
}

#[test]
fn open_in_the_gui_names_the_place_an_id_names() {
    let env = setup_multi_hunk_uncommitted_changes("file.txt");
    env.but("status -fv")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted]
┊   uv M file.txt
┊
┊╭┄ br [a-branch-1]
┊● zon author 2000-01-01 00:00:00 +0000 (sha d7681b4)
┊│     Add file
┊│     zon:u A file.txt
├╯
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);
    env.but("diff")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
─────────────────╮
 uv:7 M file.txt │
─────────────────╯

@@ -1,3 +1,4 @@
───────────────
  ┊ 1 │ +new first
1 ┊ 2 │  this
2 ┊ 3 │  is
3 ┊ 4 │  some

─────────────────╮
 uv:4 M file.txt │
─────────────────╯

@@ -7,3 +8,4 @@
───────────────
 7 ┊  8 │  with
 8 ┊  9 │  added
 9 ┊ 10 │  lines
   ┊ 11 │ +new last

"#]]);
    env.but("diff zon")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
───────────────────╮
 zo:u:f A file.txt │
───────────────────╯

@@ -1,0 +1,9 @@
───────────────
  ┊  1 │ +this
  ┊  2 │ +is
  ┊  3 │ +some
  ┊  4 │ +content
  ┊  5 │ +to
  ┊  6 │ +diff
  ┊  7 │ +with
  ┊  8 │ +added
  ┊  9 │ +lines

"#]]);

    // A branch's ID and its name name the same branch.
    for branch in ["br", "a-branch-1"] {
        env.but(format!("_open {branch} --gui --dry-run"))
            .assert()
            .success()
            .stdout_eq(snapbox::str![[r#"
[..]://open?path=[..]&t=[..]&new_window=0&to=branch&branch=refs%2Fheads%2Fa-branch-1

"#]]);
    }
    // A commit, its file and its hunk, by the commit's full hash and its change ID. A hunk opens
    // at its first added line, as `_open` opens an editor at it.
    env.but("_open zon --gui --dry-run")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
[..]://open?path=[..]&t=[..]&new_window=0&to=commit&commit=d7681b4[..]&change=zon[..]

"#]]);
    env.but("_open zon:u --gui --dry-run")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
[..]://open?path=[..]&t=[..]&new_window=0&to=commit&commit=d7681b4[..]&change=zon[..]&file=file.txt

"#]]);
    env.but("_open zo:u:f --gui --dry-run")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
[..]://open?path=[..]&t=[..]&new_window=0&to=commit&commit=d7681b4[..]&change=zon[..]&file=file.txt&line=1

"#]]);
    // Uncommitted changes, a file in them and each of its hunks.
    env.but("_open @ --gui --dry-run")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
[..]://open?path=[..]&t=[..]&new_window=0&to=uncommitted

"#]]);
    env.but("_open uv --gui --dry-run")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
[..]://open?path=[..]&t=[..]&new_window=0&to=uncommitted&file=file.txt

"#]]);
    env.but("_open uv:7 --gui --dry-run")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
[..]://open?path=[..]&t=[..]&new_window=0&to=uncommitted&file=file.txt&line=1

"#]]);
    env.but("_open uv:4 --gui --dry-run")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
[..]://open?path=[..]&t=[..]&new_window=0&to=uncommitted&file=file.txt&line=11

"#]]);
    // Tools read the link from JSON.
    env.but("--json _open zon --gui --dry-run")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
{
  "link": "[..]://open?path=[..]&t=[..]&new_window=0&to=commit&commit=d7681b4[..]&change=zon[..]"
}

"#]]);
}

#[test]
fn open_in_the_gui_names_a_linked_worktree_by_its_name() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.but("status").assert().success();
    add_dirty_worktree(&env, "wt-feature", "A");
    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊┊
┊┊╭┄ wt:@ [uncommitted] {wt-feature}
┊┊┊   nl A note.txt
┊┊├┄ wt [wt-feature] (no commits)
┊├╯
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┊╭┄ h0 [B]
┊●   lrm add B
┊│     lrm:p A B
├╯
┊
┴ 0dc3733 (common base, main, origin/main) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    // Its file and its whole uncommitted area, read from the main worktree.
    env.but("_open wt-feature:note.txt --gui --dry-run")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
[..]://open?path=[..]&t=[..]&new_window=0&to=uncommitted&worktree=wt-feature&file=note.txt

"#]]);
    env.but("_open wt:@ --gui --dry-run")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
[..]://open?path=[..]&t=[..]&new_window=0&to=uncommitted&worktree=wt-feature

"#]]);
}

#[test]
fn open_in_the_gui_names_a_remote_only_commit_by_its_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("zero-stacks");
    env.setup_metadata(&[]);
    env.file("file", "some content");
    env.but("commit -m first").assert().success();
    env.file("file", "some other content");
    env.but("commit -m second").assert().success();
    env.invoke_git("update-ref refs/remotes/origin/a-branch-1 refs/heads/a-branch-1");
    env.invoke_git("config branch.a-branch-1.remote origin");
    env.invoke_git("config branch.a-branch-1.merge refs/heads/a-branch-1");
    // The second commit stays on the remote only.
    env.but("undo").assert().success();
    env.but("discard @").assert().success();

    let remote_only = env.invoke_git("rev-parse --short=7 refs/remotes/origin/a-branch-1");
    env.but(format!("_open {remote_only} --gui --dry-run"))
        .assert()
        .success()
        .stdout_eq(format!(
            "[..]://open?path=[..]&t=[..]&new_window=0&to=upstream&branch=refs%2Fheads%2Fa-branch-1&commit={remote_only}[..]\n"
        ));
}

#[test]
fn open_in_the_gui_takes_a_links_own_parameters() {
    let env = setup_multi_hunk_uncommitted_changes("file.txt");
    // A place with no CLI ID, as a project's graph, is named as its link names it.
    env.but("_open to=project tab=graph --gui --dry-run")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
[..]://open?path=[..]&t=[..]&new_window=0&to=project&tab=graph

"#]]);
    env.but("_open to=branch branch=refs/heads/a-branch-1 file=file.txt line=4 --gui --dry-run")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
[..]://open?path=[..]&t=[..]&new_window=0&to=branch&branch=refs%2Fheads%2Fa-branch-1&file=file.txt&line=4

"#]]);
    // The GUI reads the place, so one it doesn't know still makes a link, which it says it
    // couldn't read.
    env.but("_open to=timeline --gui --dry-run")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
[..]://open?path=[..]&t=[..]&new_window=0&to=timeline

"#]]);
    // One of the link's own parameters would stand in for the link's, and an ID beside
    // parameters is refused.
    env.but("_open path=/elsewhere --gui --dry-run")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: `path` is the link's own parameter, not a place's

"#]]);
    // The help ends with every place a link opens, from the app's `LINKS.md`, whose wording is
    // the app's to change.
    env.but("_open --help")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
...
With --gui, a place with no CLI ID is given as its link's parameters, each its own argument, as but
_open to=project tab=graph --gui.

...
"#]]);
}

#[test]
fn parse_error_shows_both_ways_to_open() {
    let env = Sandbox::empty();
    // A rejected `_open` ends with examples of the editor and the GUI.
    env.but("_open --nope")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
error: unexpected argument '--nope' found

  tip: to pass '--nope' as a value, use '-- --nope'

Usage: but _open [OPTIONS] [SOURCES]...

For more information, try '--help'.

Examples:
  but _open <file-or-hunk>...            # open in an editor
  but _open <id> --gui                   # open in the GitButler GUI
  but _open <id> --gui --dry-run         # print the link instead
  but _open to=project tab=graph --gui   # a place with no CLI ID, by its link's parameters

"#]]);
}

#[test]
fn open_in_the_gui_names_a_linked_worktrees_remote_only_commit_by_its_branch() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("two-stacks");
    env.setup_metadata(&["A", "B"]);
    env.but("status").assert().success();
    // With no commit of its own, a branch's remote commits aren't listed.
    let wt = add_dirty_worktree(&env, "wt-feature", "A");
    env.invoke_git(&format!(
        "-C {} commit --allow-empty -m local-one",
        wt.display()
    ));
    let remote_only =
        env.invoke_git("commit-tree wt-feature^{tree} -p wt-feature -m only-on-the-remote");
    env.invoke_git(&format!(
        "update-ref refs/remotes/origin/wt-feature {remote_only}"
    ));
    env.invoke_git("config branch.wt-feature.remote origin");
    env.invoke_git("config branch.wt-feature.merge refs/heads/wt-feature");

    env.but(format!("_open {} --gui --dry-run", &remote_only[..7]))
        .assert()
        .success()
        .stdout_eq(format!(
            "[..]://open?path=[..]&t=[..]&new_window=0&to=upstream&branch=refs%2Fheads%2Fwt-feature&commit={remote_only}\n"
        ));
}

#[test]
fn open_in_the_gui_names_a_stack_by_its_bottom_branch() {
    let env = setup_multi_hunk_uncommitted_changes("file.txt");
    // Alone in its stack, a branch has no stack page, so it opens as that branch.
    env.but("_open g0 --gui --dry-run")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
[..]://open?path=[..]&t=[..]&new_window=0&to=branch&branch=refs%2Fheads%2Fa-branch-1

"#]]);
    env.but("branch new top-seg --above a-branch-1")
        .assert()
        .success();
    env.file("top.txt", "top");
    env.but("commit -b top-seg -m 'top commit' top.txt")
        .assert()
        .success();
    env.but("_open g0 --gui --dry-run")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
[..]://open?path=[..]&t=[..]&new_window=0&to=stack&branch=refs%2Fheads%2Fa-branch-1

"#]]);
}

#[test]
fn open_in_the_gui_takes_a_links_own_parameters_with_an_id() {
    let env = setup_multi_hunk_uncommitted_changes("file.txt");
    env.but("_open br tab=review --gui --dry-run")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
[..]://open?path=[..]&t=[..]&new_window=0&to=branch&branch=refs%2Fheads%2Fa-branch-1&tab=review

"#]]);
    // They come after what the ID names, wherever they're written.
    env.but("_open tab=review br --gui --dry-run")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
[..]://open?path=[..]&t=[..]&new_window=0&to=branch&branch=refs%2Fheads%2Fa-branch-1&tab=review

"#]]);
    env.but("_open br to=project --gui --dry-run")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: The ID already says `to=`

"#]]);
}

#[test]
fn open_in_the_gui_takes_one_place() {
    let env = setup_multi_hunk_uncommitted_changes("file.txt");
    env.file("src/a.txt", "a");
    env.but("_open zon uv --gui --dry-run")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: The GUI opens one place at a time

"#]]);
    // A path prefix names several files, which the GUI has no one place for.
    env.but("_open src/ --gui --dry-run")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Expected a branch, stack, commit, file or hunk, got a path prefix

"#]]);
    env.but("_open uv --dry-run")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
error: the following required arguments were not provided:
  --gui

Usage: but _open --gui --dry-run <SOURCES>...

For more information, try '--help'.

Examples:
  but _open <file-or-hunk>...            # open in an editor
  but _open <id> --gui                   # open in the GitButler GUI
  but _open <id> --gui --dry-run         # print the link instead
  but _open to=project tab=graph --gui   # a place with no CLI ID, by its link's parameters

"#]]);
}

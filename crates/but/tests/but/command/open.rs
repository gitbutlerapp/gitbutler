use but_api::open::program::USER_DEFINED_PROGRAMS_FILENAME;

use crate::utils::Sandbox;

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
╭┄ zz [uncommitted]
┊   xk A new-file.txt
┊
┴ 0dc3733 (common base) 2000-01-02 add M

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
────────────────────────────╮
rn:7 file-with-additions.txt│
────────────────────────────╯
     1│+new first
   1 2│ this
   2 3│ is
   3 4│ some
────────────────────────────╮
rn:4 file-with-additions.txt│
────────────────────────────╯
    7  8│ with
    8  9│ added
    9 10│ lines
      11│+new last
────────────────────────────╮
rw:b file-with-deletions.txt│
────────────────────────────╯
   1  │-this
   2 1│ is
   3 2│ some
   4 3│ content
────────────────────────────╮
rw:6 file-with-deletions.txt│
────────────────────────────╯
    6  5│ diff
    7  6│ with
    8  7│ added
    9   │-lines
────────────────────────╮
lp:6 file-with-mixed.txt│
────────────────────────╯
    1  1│ this
    2   │-is
       2│+IS
    3  3│ some
    4  4│ content
    5  5│ to
    6  6│ diff
    7  7│ with
    8   │-added
    9   │-lines
       8│+ADDED

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
╭┄ zz [uncommitted]
┊   pv M file with some $meta; cat A > new-file.txt; spaces in it.txt
┊
┊╭┄ br [a-branch-1]
┊●   1 Add file
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

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
Error: Could not find uncommitted change: 'notexist'

Hint: Run `but status` for applicable targets.

"#]]);
}

#[test]
fn cannot_open_committed_changes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("status -f")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊●   tpm add A
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("_open A -p echo")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Expected uncommitted file or hunk, got a branch

"#]]);

    env.but("_open tpm -p echo")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Expected uncommitted file or hunk, got a commit

"#]]);

    env.but("_open tpm:t -p echo")
        .assert()
        .failure()
        .stderr_eq(snapbox::str![[r#"
Error: Expected uncommitted file or hunk, got a committed file

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
╭┄ zz [uncommitted]
┊   pv M file with some $meta; cat A > new-file.txt; spaces in it.txt
┊
┊╭┄ br [a-branch-1]
┊●   1 Add file
┊│     1:p A file with some $meta; cat A > new-file.txt; spaces in it.txt
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    env.but("_open pv -p test-program").assert().success().stdout_eq(snapbox::str![[r#"
Test Program - Open File: filepath='/[..]/file with some $meta; cat A > new-file.txt; spaces in it.txt'

"#]]);

    env.but("diff")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
─────────────────────────────────────────────────────────────────╮
pv:7 file with some $meta; cat A > new-file.txt; spaces in it.txt│
─────────────────────────────────────────────────────────────────╯
     1│+new first
   1 2│ this
   2 3│ is
   3 4│ some
─────────────────────────────────────────────────────────────────╮
pv:4 file with some $meta; cat A > new-file.txt; spaces in it.txt│
─────────────────────────────────────────────────────────────────╯
    7  8│ with
    8  9│ added
    9 10│ lines
      11│+new last

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
╭┄ zz [uncommitted]
┊   uv M file.txt
┊
┊╭┄ br [a-branch-1]
┊●   1 Add file
┊│     1:u A file.txt
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but diff` to see uncommitted changes and `but commit -b <branch> -m "message" <id>` to commit them

"#]]);

    env.but("diff")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
─────────────╮
uv:7 file.txt│
─────────────╯
     1│+new first
   1 2│ this
   2 3│ is
   3 4│ some
─────────────╮
uv:4 file.txt│
─────────────╯
    7  8│ with
    8  9│ added
    9 10│ lines
      11│+new last

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
╭┄ zz [uncommitted]
┊   tt A Dockerfile
┊   zn A file.md
┊   uv A file.txt
┊
┴ 0dc3733 (common base) 2000-01-02 add M

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
╭┄ zz [uncommitted]
┊   tt A Dockerfile
┊   zn A file.md
┊   ul A file.md.touch
┊   uv A file.txt
┊
┴ 0dc3733 (common base) 2000-01-02 add M

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
╭┄ zz [uncommitted]
┊   tt A Dockerfile
┊   mu A Dockerfile.touch
┊   zn A file.md
┊   ul A file.md.touch
┊   uv A file.txt
┊
┴ 0dc3733 (common base) 2000-01-02 add M

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

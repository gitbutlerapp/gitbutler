use gitbutler_core::{git::diff, virtual_branches::context::hunk_with_context};

#[test]
fn replace_line_mid_file() {
    let hunk_diff = r#"@@ -8 +8 @@ default = ["serde", "rusqlite"]
-serde = ["dep:serde", "uuid/serde"]
+SERDE = ["dep:serde", "uuid/serde"]
"#;
    let with_ctx = hunk_with_context(
        hunk_diff,
        8,
        8,
        false,
        3,
        &file_lines(),
        diff::ChangeType::Added,
    );
    let expected = r#"@@ -5,7 +5,7 @@

 [features]
 default = ["serde", "rusqlite"]
-serde = ["dep:serde", "uuid/serde"]
+SERDE = ["dep:serde", "uuid/serde"]
 rusqlite = ["dep:rusqlite"]

 [dependencies]
"#;
    assert_eq!(with_ctx.diff.replace("\n \n", "\n\n"), expected);
    assert_eq!(with_ctx.old_start, 5);
    assert_eq!(with_ctx.old_lines, 7);
    assert_eq!(with_ctx.new_start, 5);
    assert_eq!(with_ctx.new_lines, 7);
}

#[test]
fn replace_line_top_file() {
    let hunk_diff = r#"@@ -2 +2 @@
-name = "gitbutler-core"
+NAME = "gitbutler-core"
"#;
    let with_ctx = hunk_with_context(
        hunk_diff,
        2,
        2,
        false,
        3,
        &file_lines(),
        diff::ChangeType::Added,
    );
    assert_eq!(
        with_ctx.diff.replace("\n \n", "\n\n"),
        r#"@@ -1,5 +1,5 @@
 [package]
-name = "gitbutler-core"
+NAME = "gitbutler-core"
 version = "0.0.0"
 edition = "2021"

"#
    );
    assert_eq!(with_ctx.old_start, 1);
    assert_eq!(with_ctx.old_lines, 5);
    assert_eq!(with_ctx.new_start, 1);
    assert_eq!(with_ctx.new_lines, 5);
}

#[test]
fn replace_line_start_file() {
    let hunk_diff = "@@ -1 +1 @@
-[package]
+[PACKAGE]
";
    let with_ctx = hunk_with_context(
        hunk_diff,
        1,
        1,
        false,
        3,
        &file_lines(),
        diff::ChangeType::Added,
    );
    assert_eq!(
        with_ctx.diff.replace("\n \n", "\n\n"),
        r#"@@ -1,4 +1,4 @@
-[package]
+[PACKAGE]
 name = "gitbutler-core"
 version = "0.0.0"
 edition = "2021"
"#
    );
    assert_eq!(with_ctx.old_start, 1);
    assert_eq!(with_ctx.old_lines, 4);
    assert_eq!(with_ctx.new_start, 1);
    assert_eq!(with_ctx.new_lines, 4);
}

#[test]
fn replace_line_bottom_file() {
    let hunk_diff = "@@ -13 +13 @@
-serde = { workspace = true, optional = true }
+SERDE = { workspace = true, optional = true }
";
    let with_ctx = hunk_with_context(
        hunk_diff,
        13,
        13,
        false,
        3,
        &file_lines(),
        diff::ChangeType::Added,
    );
    assert_eq!(
        with_ctx.diff.replace("\n \n", "\n\n"),
        r#"@@ -10,5 +10,5 @@

 [dependencies]
 rusqlite = { workspace = true, optional = true }
-serde = { workspace = true, optional = true }
+SERDE = { workspace = true, optional = true }
 uuid = { workspace = true, features = ["v4", "fast-rng"] }
"#
    );
    assert_eq!(with_ctx.old_start, 10);
    assert_eq!(with_ctx.old_lines, 5);
    assert_eq!(with_ctx.new_start, 10);
    assert_eq!(with_ctx.new_lines, 5);
}

#[test]
fn replace_with_more_lines() {
    let hunk_diff = r#"@@ -8 +8,4 @@
-serde = ["dep:serde", "uuid/serde"]
+one
+two
+three
+four
"#;
    let with_ctx = hunk_with_context(
        hunk_diff,
        8,
        8,
        false,
        3,
        &file_lines(),
        diff::ChangeType::Added,
    );
    assert_eq!(
        with_ctx.diff.replace("\n \n", "\n\n"),
        r#"@@ -5,7 +5,10 @@

 [features]
 default = ["serde", "rusqlite"]
-serde = ["dep:serde", "uuid/serde"]
+one
+two
+three
+four
 rusqlite = ["dep:rusqlite"]

 [dependencies]
"#
    );
    assert_eq!(with_ctx.old_start, 5);
    assert_eq!(with_ctx.old_lines, 7);
    assert_eq!(with_ctx.new_start, 5);
    assert_eq!(with_ctx.new_lines, 10);
}

#[test]
fn replace_with_less_lines() {
    let hunk_diff = r#"@@ -7,3 +7 @@
-default = ["serde", "rusqlite"]
-serde = ["dep:serde", "uuid/serde"]
-rusqlite = ["dep:rusqlite"]
+foo = ["foo"]
"#;
    let with_ctx = hunk_with_context(
        hunk_diff,
        7,
        7,
        false,
        3,
        &file_lines(),
        diff::ChangeType::Added,
    );
    assert_eq!(
        with_ctx.diff.replace("\n \n", "\n\n"),
        r#"@@ -4,9 +4,7 @@
 edition = "2021"

 [features]
-default = ["serde", "rusqlite"]
-serde = ["dep:serde", "uuid/serde"]
-rusqlite = ["dep:rusqlite"]
+foo = ["foo"]

 [dependencies]
 rusqlite = { workspace = true, optional = true }
"#
    );
    assert_eq!(with_ctx.old_start, 4);
    assert_eq!(with_ctx.old_lines, 9);
    assert_eq!(with_ctx.new_start, 4);
    assert_eq!(with_ctx.new_lines, 7);
}

#[test]
fn empty_string_doesnt_panic() {
    let hunk_diff = "";
    let with_ctx = hunk_with_context(
        hunk_diff,
        1,
        1,
        false,
        3,
        &file_lines(),
        diff::ChangeType::Added,
    );
    assert_eq!(with_ctx.diff, "");
}

#[test]
fn removed_file() {
    let hunk_diff = r#"@@ -1,14 +0,0 @@
-[package]
-name = "gitbutler-core"
-version = "0.0.0"
-edition = "2021"
-
-[features]
-default = ["serde", "rusqlite"]
-serde = ["dep:serde", "uuid/serde"]
-rusqlite = ["dep:rusqlite"]
-
-[dependencies]
-rusqlite = { workspace = true, optional = true }
-serde = { workspace = true, optional = true }
-uuid = { workspace = true, features = ["v4", "fast-rng"] }
"#;
    let with_ctx = hunk_with_context(
        hunk_diff,
        1,
        0,
        false,
        3,
        &file_lines(),
        diff::ChangeType::Added,
    );
    assert_eq!(with_ctx.diff.replace("\n \n", "\n\n"), hunk_diff);
    assert_eq!(with_ctx.old_start, 1);
    assert_eq!(with_ctx.old_lines, 14);
    assert_eq!(with_ctx.new_start, 0);
    assert_eq!(with_ctx.new_lines, 0);
}
#[test]
fn new_file() {
    let hunk_diff = "@@ -0,0 +1,5 @@
+line 1
+line 2
+line 3
+line 4
+line 5
";
    let with_ctx = hunk_with_context(
        hunk_diff,
        0,
        1,
        false,
        3,
        &Vec::new(),
        diff::ChangeType::Added,
    );
    assert_eq!(with_ctx.diff.replace("\n \n", "\n\n"), hunk_diff);
    assert_eq!(with_ctx.old_start, 0);
    assert_eq!(with_ctx.old_lines, 0);
    assert_eq!(with_ctx.new_start, 1);
    assert_eq!(with_ctx.new_lines, 5);
}

#[test]
fn only_add_lines() {
    let hunk_diff = "@@ -8,0 +9,3 @@
+one
+two
+three
";
    let with_ctx = hunk_with_context(
        hunk_diff,
        8,
        9,
        false,
        3,
        &file_lines(),
        diff::ChangeType::Added,
    );
    let expected = r#"@@ -6,6 +6,9 @@
 [features]
 default = ["serde", "rusqlite"]
 serde = ["dep:serde", "uuid/serde"]
+one
+two
+three
 rusqlite = ["dep:rusqlite"]

 [dependencies]
"#;
    assert_eq!(with_ctx.diff.replace("\n \n", "\n\n"), expected);
    assert_eq!(with_ctx.old_start, 6);
    assert_eq!(with_ctx.old_lines, 6);
    assert_eq!(with_ctx.new_start, 6);
    assert_eq!(with_ctx.new_lines, 9);
}

#[test]
fn only_add_lines_with_additions_below() {
    let hunk_diff = "@@ -8,0 +13,3 @@
+one
+two
+three
";
    let with_ctx = hunk_with_context(
        hunk_diff,
        8,
        13,
        false,
        3,
        &file_lines(),
        diff::ChangeType::Added,
    );
    let expected = r#"@@ -6,6 +10,9 @@
 [features]
 default = ["serde", "rusqlite"]
 serde = ["dep:serde", "uuid/serde"]
+one
+two
+three
 rusqlite = ["dep:rusqlite"]

 [dependencies]
"#;
    assert_eq!(with_ctx.diff.replace("\n \n", "\n\n"), expected);
    assert_eq!(with_ctx.old_start, 6);
    assert_eq!(with_ctx.old_lines, 6);
    assert_eq!(with_ctx.new_start, 10);
    assert_eq!(with_ctx.new_lines, 9);
}

#[test]
fn only_remove_lines() {
    let hunk_diff = r#"@@ -7,3 +6,0 @@
-default = ["serde", "rusqlite"]
-serde = ["dep:serde", "uuid/serde"]
-rusqlite = ["dep:rusqlite"]
"#;
    let expected = r#"@@ -4,9 +4,6 @@
 edition = "2021"

 [features]
-default = ["serde", "rusqlite"]
-serde = ["dep:serde", "uuid/serde"]
-rusqlite = ["dep:rusqlite"]

 [dependencies]
 rusqlite = { workspace = true, optional = true }
"#;
    let with_ctx = hunk_with_context(
        hunk_diff,
        7,
        6,
        false,
        3,
        &file_lines(),
        diff::ChangeType::Added,
    );
    assert_eq!(with_ctx.diff.replace("\n \n", "\n\n"), expected);
    assert_eq!(with_ctx.old_start, 4);
    assert_eq!(with_ctx.old_lines, 9);
    assert_eq!(with_ctx.new_start, 4);
    assert_eq!(with_ctx.new_lines, 6);
}

#[test]
fn only_remove_lines_with_additions_below() {
    let hunk_diff = r#"@@ -7,3 +10,0 @@
-default = ["serde", "rusqlite"]
-serde = ["dep:serde", "uuid/serde"]
-rusqlite = ["dep:rusqlite"]
"#;
    let expected = r#"@@ -4,9 +8,6 @@
 edition = "2021"

 [features]
-default = ["serde", "rusqlite"]
-serde = ["dep:serde", "uuid/serde"]
-rusqlite = ["dep:rusqlite"]

 [dependencies]
 rusqlite = { workspace = true, optional = true }
"#;
    let with_ctx = hunk_with_context(
        hunk_diff,
        7,
        10,
        false,
        3,
        &file_lines(),
        diff::ChangeType::Added,
    );
    assert_eq!(with_ctx.diff.replace("\n \n", "\n\n"), expected);
    assert_eq!(with_ctx.old_start, 4);
    assert_eq!(with_ctx.old_lines, 9);
    assert_eq!(with_ctx.new_start, 8);
    assert_eq!(with_ctx.new_lines, 6);
}

#[test]
fn weird_testcase() {
    let hunk_diff = "@@ -11,2 +10,0 @@
-
-    @waiting_users = User.where(approved: false).count
";
    let with_ctx = hunk_with_context(
        hunk_diff,
        11,
        10,
        false,
        3,
        &file_lines_2(),
        diff::ChangeType::Added,
    );
    let expected = "@@ -8,8 +8,6 @@
                                  .order(:created_at)
                                  .page params[:page]
     @total = @registrations.total_count
-
-    @waiting_users = User.where(approved: false).count
   end

   def invite
";
    assert_eq!(with_ctx.diff.replace("\n \n", "\n\n"), expected);
    assert_eq!(with_ctx.old_start, 8);
    assert_eq!(with_ctx.old_lines, 8);
    assert_eq!(with_ctx.new_start, 8);
    assert_eq!(with_ctx.new_lines, 6);
}

#[test]
fn new_line_added() {
    let hunk_diff = "@@ -2,0 +3 @@ alias(
+    newstuff
";
    let with_ctx = hunk_with_context(
        hunk_diff,
        2,
        3,
        false,
        3,
        &file_lines_3(),
        diff::ChangeType::Added,
    );
    let expected = r#"@@ -1,4 +1,5 @@
 alias(
     name = "rdeps",
+    newstuff
     actual = "//java/com/videlov/rdeps:rdeps",
 )
"#;
    assert_eq!(with_ctx.diff, expected);
}

fn file_lines() -> Vec<&'static str> {
    let file_lines_before = r#"[package]
name = "gitbutler-core"
version = "0.0.0"
edition = "2021"

[features]
default = ["serde", "rusqlite"]
serde = ["dep:serde", "uuid/serde"]
rusqlite = ["dep:rusqlite"]

[dependencies]
rusqlite = { workspace = true, optional = true }
serde = { workspace = true, optional = true }
uuid = { workspace = true, features = ["v4", "fast-rng"] }
"#;
    file_lines_before.lines().collect::<Vec<_>>()
}

fn file_lines_2() -> Vec<&'static str> {
    let file_lines_before = r#"class Admin::WaitingController < Admin::AdminController
  def index
    @registrations = Registration.where(invited_at: nil)
    if params[:q]
      @registrations = @registrations.where("email LIKE ?", "%#{params[:q]}%")
    end
    @registrations = @registrations.includes(:invite_code)
                                 .order(:created_at)
                                 .page params[:page]
    @total = @registrations.total_count

    @waiting_users = User.where(approved: false).count
  end

  def invite
    if params[:id]
      @registrations = Registration.where(id: params[:id])
"#;
    file_lines_before.lines().collect::<Vec<_>>()
}

fn file_lines_3() -> Vec<&'static str> {
    let file_lines_before = r#"alias(
    name = "rdeps",
    actual = "//java/com/videlov/rdeps:rdeps",
)
"#;
    file_lines_before.lines().collect::<Vec<_>>()
}

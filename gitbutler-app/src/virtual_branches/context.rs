use crate::git::diff;
use anyhow::{Context, Result};

pub fn hunk_with_context(
    hunk_diff: &str,
    hunk_old_start_line: usize,
    hunk_new_start_line: usize,
    is_binary: bool,
    context_lines: usize,
    file_lines_before: &[&str],
    change_type: diff::ChangeType,
) -> Result<diff::Hunk> {
    let diff_lines = hunk_diff
        .lines()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>();
    if diff_lines.is_empty() {
        #[allow(clippy::cast_possible_truncation)]
        return Ok(diff::Hunk {
            diff: hunk_diff.to_owned(),
            old_start: hunk_old_start_line as u32,
            old_lines: 0,
            new_start: hunk_new_start_line as u32,
            new_lines: 0,
            binary: is_binary,
            change_type,
        });
    }

    let new_file = hunk_old_start_line == 0;
    let deleted_file = hunk_new_start_line == 0;

    let removed_count = diff_lines
        .iter()
        .filter(|line| line.starts_with('-'))
        .count();
    let added_count = diff_lines
        .iter()
        .filter(|line| line.starts_with('+'))
        .count();

    // Get context lines before the diff
    let mut context_before = Vec::new();
    let before_context_ending_index = if removed_count == 0 { // Compensate for when the removed_count is 0
        hunk_old_start_line
    } else {
        hunk_old_start_line.saturating_sub(1)
    };
    let before_context_starting_index = before_context_ending_index.saturating_sub(context_lines);

    for index in before_context_starting_index..before_context_ending_index {
        if let Some(l) = file_lines_before.get(index) {
            let mut s = (*l).to_string();
            s.insert(0, ' ');
            context_before.push(s);
        }
    }

    // Get context lines after the diff
    let mut context_after = Vec::new();
    let after_context_starting_index = before_context_ending_index + removed_count;
    let after_context_ending_index = after_context_starting_index + context_lines;

    for index in after_context_starting_index..after_context_ending_index {
        if let Some(l) = file_lines_before.get(index) {
            let mut s = (*l).to_string();
            s.insert(0, ' ');
            context_after.push(s);
        }
    }

    let start_line_before = if new_file { // If we've created a new file, start_line_before should be 0
        0
    } else {
        before_context_starting_index + 1
    };

    let start_line_after = if deleted_file { // If we've deleted a new file, start_line_after should be 0
        0
    } else if added_count == 0 { // Compensate for when the added_count is 0
        hunk_new_start_line.saturating_sub(context_before.len()) + 1
    } else {
        hunk_new_start_line.saturating_sub(context_before.len())
    };

    let line_count_before = removed_count + context_before.len() + context_after.len();
    let line_count_after = added_count + context_before.len() + context_after.len();
    let header = format!(
        "@@ -{},{} +{},{} @@",
        start_line_before, line_count_before, start_line_after, line_count_after
    );

    let body = &diff_lines[1..];
    // Update unidiff body with context lines
    let mut b = Vec::new();
    b.extend(context_before.clone());
    b.extend_from_slice(body);
    b.extend(context_after.clone());
    let body = b;

    // Construct a new diff with updated header and body
    let mut diff_lines = Vec::new();
    diff_lines.push(header);
    diff_lines.extend(body);
    let mut diff = diff_lines.join("\n");
    // Add trailing newline
    diff.push('\n');

    #[allow(clippy::cast_possible_truncation)]
    let hunk = diff::Hunk {
        diff,
        old_start: start_line_before as u32,
        old_lines: line_count_before as u32,
        new_start: start_line_after as u32,
        new_lines: line_count_after as u32,
        binary: is_binary,
        change_type,
    };
    Ok(hunk)
}

#[cfg(test)]
mod tests {
    use super::*;
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
        )
        .unwrap();
        let expected = r#"@@ -5,7 +5,7 @@
 
 [features]
 default = ["serde", "rusqlite"]
-serde = ["dep:serde", "uuid/serde"]
+SERDE = ["dep:serde", "uuid/serde"]
 rusqlite = ["dep:rusqlite"]
 
 [dependencies]
"#;
        assert_eq!(with_ctx.diff, expected);
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
        )
        .unwrap();
        assert_eq!(
            with_ctx.diff,
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
        )
        .unwrap();
        assert_eq!(
            with_ctx.diff,
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
        )
        .unwrap();
        assert_eq!(
            with_ctx.diff,
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
        )
        .unwrap();
        assert_eq!(
            with_ctx.diff,
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
        )
        .unwrap();
        assert_eq!(
            with_ctx.diff,
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
        )
        .unwrap();
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
        )
        .unwrap();
        assert_eq!(with_ctx.diff, hunk_diff);
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
        )
        .unwrap();
        assert_eq!(with_ctx.diff, hunk_diff);
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
        )
        .unwrap();
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
        assert_eq!(with_ctx.diff, expected);
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
        )
        .unwrap();
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
        assert_eq!(with_ctx.diff, expected);
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
        )
        .unwrap();
        assert!(with_ctx.diff == expected);
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
        )
        .unwrap();
        assert_eq!(with_ctx.diff, expected);
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
        )
        .unwrap();
        let expected = "@@ -8,8 +8,6 @@
                                  .order(:created_at)
                                  .page params[:page]
     @total = @registrations.total_count
-
-    @waiting_users = User.where(approved: false).count
   end
 
   def invite
";
        assert_eq!(with_ctx.diff, expected);
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
        )
        .unwrap();
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
}

use bstr::ByteSlice;

use super::app::{CommitDiffFile, CommitDiffLine, DiffLineKind, LazyApp, Panel};

impl LazyApp {
    pub(super) fn open_diff_modal(&mut self) {
        if !matches!(self.active_panel, Panel::Status) {
            self.command_log
                .push("Select a commit in the Status panel to view its diff".to_string());
            return;
        }

        let Some(commit) = self.get_selected_commit().cloned() else {
            self.command_log
                .push("No commit selected to diff".to_string());
            return;
        };

        let (changes, _) = match self.get_commit_file_changes(&commit.full_id) {
            Ok(result) => result,
            Err(e) => {
                self.command_log
                    .push(format!("Failed to load commit changes: {}", e));
                return;
            }
        };

        let mut files = Vec::new();
        for change in changes {
            let path = change.path.to_string();
            let status = change.status.clone();
            let diff_result =
                but_api::legacy::diff::tree_change_diffs(self.project_id, change.clone());

            let lines = match diff_result {
                Ok(patch) => Self::lines_from_patch(patch),
                Err(e) => {
                    self.command_log
                        .push(format!("Failed to load diff for {}: {}", path, e));
                    vec![CommitDiffLine {
                        text: format!("Error loading diff: {}", e),
                        kind: DiffLineKind::Info,
                    }]
                }
            };

            files.push(CommitDiffFile {
                path,
                status,
                lines,
            });
        }

        if files.is_empty() {
            self.command_log
                .push("Commit has no file changes".to_string());
            return;
        }

        self.diff_modal_files = files;
        self.diff_modal_selected_file = 0;
        self.diff_modal_scroll = 0;
        self.show_diff_modal = true;
        self.command_log
            .push(format!("Viewing diff for commit {}", commit.id));
    }

    pub(super) fn close_diff_modal(&mut self) {
        self.show_diff_modal = false;
        self.diff_modal_files.clear();
        self.diff_modal_selected_file = 0;
        self.diff_modal_scroll = 0;
    }

    pub(super) fn scroll_diff_modal(&mut self, delta: i16) {
        if delta > 0 {
            self.diff_modal_scroll = self.diff_modal_scroll.saturating_add(delta as u16);
        } else if delta < 0 {
            self.diff_modal_scroll = self.diff_modal_scroll.saturating_sub((-delta) as u16);
        }
        self.clamp_diff_scroll();
    }

    pub(super) fn select_next_diff_file(&mut self) {
        if self.diff_modal_files.is_empty() {
            return;
        }
        self.diff_modal_selected_file =
            (self.diff_modal_selected_file + 1) % self.diff_modal_files.len();
        self.diff_modal_scroll = 0;
    }

    pub(super) fn select_prev_diff_file(&mut self) {
        if self.diff_modal_files.is_empty() {
            return;
        }

        if self.diff_modal_selected_file == 0 {
            self.diff_modal_selected_file = self.diff_modal_files.len() - 1;
        } else {
            self.diff_modal_selected_file -= 1;
        }
        self.diff_modal_scroll = 0;
    }

    pub(super) fn jump_diff_hunk_forward(&mut self) {
        self.jump_diff_hunk(true);
    }

    pub(super) fn jump_diff_hunk_backward(&mut self) {
        self.jump_diff_hunk(false);
    }

    fn jump_diff_hunk(&mut self, forward: bool) {
        if self.diff_modal_files.is_empty() {
            return;
        }

        let selected_idx = self
            .diff_modal_selected_file
            .min(self.diff_modal_files.len().saturating_sub(1));
        let Some(file) = self.diff_modal_files.get(selected_idx) else {
            return;
        };

        let headers: Vec<usize> = file
            .lines
            .iter()
            .enumerate()
            .filter_map(|(idx, line)| {
                if matches!(line.kind, DiffLineKind::Header) {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect();

        if headers.is_empty() {
            return;
        }

        let current = self.diff_modal_scroll as usize;
        let target = if forward {
            headers
                .iter()
                .copied()
                .find(|idx| *idx > current)
                .unwrap_or(headers[0])
        } else {
            headers
                .iter()
                .rev()
                .copied()
                .find(|idx| *idx < current)
                .unwrap_or(*headers.last().unwrap())
        };

        self.diff_modal_scroll = target as u16;
        self.clamp_diff_scroll();
    }

    fn lines_from_patch(patch: Option<but_core::UnifiedPatch>) -> Vec<CommitDiffLine> {
        match patch {
            Some(but_core::UnifiedPatch::Patch { hunks, .. }) => {
                let mut lines = Vec::new();
                for hunk in hunks {
                    for diff_line in hunk.diff.lines() {
                        let text = String::from_utf8_lossy(diff_line).to_string();
                        let kind = if text.starts_with("@@") {
                            DiffLineKind::Header
                        } else if text.starts_with('+') {
                            DiffLineKind::Added
                        } else if text.starts_with('-') {
                            DiffLineKind::Removed
                        } else {
                            DiffLineKind::Context
                        };
                        lines.push(CommitDiffLine { text, kind });
                    }
                    lines.push(CommitDiffLine {
                        text: String::new(),
                        kind: DiffLineKind::Context,
                    });
                }
                lines
            }
            Some(but_core::UnifiedPatch::Binary) => vec![CommitDiffLine {
                text: "Binary file (diff unavailable)".to_string(),
                kind: DiffLineKind::Info,
            }],
            Some(but_core::UnifiedPatch::TooLarge { size_in_bytes }) => vec![CommitDiffLine {
                text: format!("File too large to diff ({} bytes)", size_in_bytes),
                kind: DiffLineKind::Info,
            }],
            None => vec![CommitDiffLine {
                text: "Diff not available".to_string(),
                kind: DiffLineKind::Info,
            }],
        }
    }

    fn clamp_diff_scroll(&mut self) {
        let Some(file) = self.diff_modal_files.get(
            self.diff_modal_selected_file
                .min(self.diff_modal_files.len().saturating_sub(1)),
        ) else {
            self.diff_modal_scroll = 0;
            return;
        };
        let max_scroll = file.lines.len().saturating_sub(1) as u16;
        if self.diff_modal_scroll > max_scroll {
            self.diff_modal_scroll = max_scroll;
        }
    }
}

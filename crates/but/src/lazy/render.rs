use bstr::ByteSlice;
use gitbutler_branch_actions::upstream_integration::{
    BranchStatus as UpstreamBranchStatus, StackStatuses,
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};
use std::{cmp, collections::BTreeSet};

use super::app::{CommitModalFocus, DiffLineKind, LazyApp, Panel, RewordModalFocus};
use crate::status::assignment::FileAssignment;

pub(super) fn ui(f: &mut Frame, app: &mut LazyApp) {
    let size = f.area();

    // Create outer layout: main content area and status line at bottom
    let outer_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(size);

    // Create main layout: left side (panels) and right side (main view + command log)
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(outer_chunks[0]);

    // Left side: split into three panels (Upstream, Status, Oplog)
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(if app.upstream_info.is_some() { 3 } else { 0 }),
            Constraint::Min(10),
            Constraint::Percentage(25),
        ])
        .split(main_chunks[0]);

    // Right side: main view (top) and command log (bottom) when visible
    let right_chunks = if app.command_log_visible {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(80), Constraint::Percentage(20)])
            .split(main_chunks[1])
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(100)])
            .split(main_chunks[1])
    };

    // Store panel areas for mouse click detection
    if app.upstream_info.is_some() {
        app.upstream_area = Some(left_chunks[0]);
        render_upstream(f, app, left_chunks[0]);
    } else {
        app.upstream_area = None;
    }

    app.status_area = Some(left_chunks[1]);
    app.oplog_area = Some(left_chunks[2]);

    // Render status panel (combined unassigned files and branches)
    render_status(f, app, left_chunks[1]);

    // Render oplog panel
    render_oplog(f, app, left_chunks[2]);

    // Store details area for mouse click detection
    app.details_area = Some(right_chunks[0]);

    // Render main view
    render_main_view(f, app, right_chunks[0]);

    // Render command log if visible
    if app.command_log_visible {
        render_command_log(f, app, right_chunks[1]);
    }

    // Render status line at the bottom
    render_status_line(f, app, outer_chunks[1]);

    // Render help modal if shown
    if app.show_help {
        render_help_modal(f, app, size);
    }

    // Render commit modal if shown
    if app.show_commit_modal {
        render_commit_modal(f, app, size);
    }

    if app.show_reword_modal {
        render_reword_modal(f, app, size);
    }

    if app.show_uncommit_modal {
        render_uncommit_modal(f, app, size);
    }

    if app.show_absorb_modal {
        render_absorb_modal(f, app, size);
    }

    if app.show_squash_modal {
        render_squash_modal(f, app, size);
    }

    if app.show_diff_modal {
        render_diff_modal(f, app, size);
    }

    if app.show_branch_rename_modal {
        render_branch_rename_modal(f, app, size);
    }

    if app.show_update_modal {
        render_update_modal(f, app, size);
    }

    if app.show_restore_modal {
        render_restore_modal(f, app, size);
    }
}

fn render_upstream(f: &mut Frame, app: &mut LazyApp, area: Rect) {
    let is_active = matches!(app.active_panel, Panel::Upstream);
    let border_style = if is_active {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    if let Some(upstream) = &app.upstream_info {
        // Calculate relative time for last fetched
        let last_fetched_text = upstream
            .last_fetched_ms
            .map(|ms| {
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis();
                let elapsed_ms = now_ms.saturating_sub(ms);
                let elapsed_secs = elapsed_ms / 1000;

                if elapsed_secs < 60 {
                    format!("{}s ago", elapsed_secs)
                } else if elapsed_secs < 3600 {
                    let minutes = elapsed_secs / 60;
                    format!("{}m ago", minutes)
                } else if elapsed_secs < 86400 {
                    let hours = elapsed_secs / 3600;
                    format!("{}h ago", hours)
                } else {
                    let days = elapsed_secs / 86400;
                    format!("{}d ago", days)
                }
            })
            .unwrap_or_else(|| "unknown".to_string());

        let mut content = vec![Line::from(vec![
            Span::styled("⏫ ", Style::default().fg(Color::Yellow)),
            Span::styled(
                format!("{} new commits", upstream.behind_count),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" • "),
            Span::styled("fetched ", Style::default().fg(Color::DarkGray)),
            Span::styled(last_fetched_text, Style::default().fg(Color::Cyan)),
        ])];

        if let Some(statuses) = &app.upstream_integration_status {
            content.push(Line::from(""));
            content.push(Line::from(vec![Span::styled(
                "Applied Branch Status",
                Style::default().add_modifier(Modifier::BOLD),
            )]));
            match statuses {
                StackStatuses::UpToDate => {
                    content.push(Line::from(vec![Span::styled(
                        "✅ All applied branches are up to date",
                        Style::default().fg(Color::Green),
                    )]));
                }
                StackStatuses::UpdatesRequired {
                    worktree_conflicts,
                    statuses,
                } => {
                    if !worktree_conflicts.is_empty() {
                        content.push(Line::from(vec![Span::styled(
                            "❗️ Uncommitted changes may conflict with an update",
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                        )]));
                        for conflict in worktree_conflicts.iter().take(3) {
                            content.push(Line::from(vec![Span::raw(format!(
                                "   • {}",
                                conflict.to_string()
                            ))]));
                        }
                        if worktree_conflicts.len() > 3 {
                            content.push(Line::from("   • ..."));
                        }
                    }

                    for (stack_id, stack_status) in statuses {
                        if let Some(name) = stack_id
                            .and_then(|id| app.stacks.iter().find(|s| s.id == Some(id)))
                            .map(|s| s.name.clone())
                        {
                            content.push(Line::from(vec![Span::styled(
                                format!("Stack: {name}"),
                                Style::default().fg(Color::Cyan),
                            )]));
                        }
                        for branch in &stack_status.branch_statuses {
                            let (icon, label, style) = branch_status_summary(&branch.status);
                            content.push(Line::from(vec![
                                Span::raw("   "),
                                Span::raw(icon.to_string()),
                                Span::raw(" "),
                                Span::styled(
                                    branch.name.clone(),
                                    Style::default().fg(Color::White),
                                ),
                                Span::raw(": "),
                                Span::styled(label.to_string(), style),
                            ]));
                        }
                    }
                }
            }

            content.push(Line::from(""));
            content.push(Line::from(vec![Span::styled(
                "Press 'u' to update applied branches",
                Style::default().fg(Color::Yellow),
            )]));
        }

        let paragraph = Paragraph::new(content).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title("Upstream [1]"),
        );

        f.render_widget(paragraph, area);
    }
}

fn render_status(f: &mut Frame, app: &mut LazyApp, area: Rect) {
    let is_active = matches!(app.active_panel, Panel::Status);
    let border_style = if is_active {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let mut items: Vec<ListItem> = Vec::new();

    let selected_lock_ids = selected_lock_ids(app);

    // Add unassigned files section
    if !app.unassigned_files.is_empty() {
        items.push(ListItem::new(Line::from(vec![Span::styled(
            "Unassigned Files",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )])));

        for file in &app.unassigned_files {
            let path = file.path.to_string();
            let mut spans = vec![
                Span::raw("  "),
                Span::styled(path, Style::default().fg(Color::Yellow)),
            ];
            if let Some(lock_spans) = file_lock_spans(file) {
                spans.push(Span::raw(" "));
                spans.extend(lock_spans);
            }
            items.push(ListItem::new(Line::from(spans)));
        }
    }

    if !app.unassigned_files.is_empty() {
        items.push(ListItem::new(Line::from(""))); // Blank line after unassigned files
    }

    // Add stacks section
    for stack in &app.stacks {
        let branch_count = stack.branches.len();

        // Add branches in stack
        for (branch_idx, branch) in stack.branches.iter().enumerate() {
            let is_last_branch = branch_idx == branch_count - 1;

            // Determine branch line prefix based on position in stack
            let branch_prefix = if branch_count > 1 {
                if branch_idx == 0 {
                    "╭─ " // First branch
                } else if is_last_branch {
                    "╰─ " // Last branch
                } else {
                    "├─ " // Middle branch
                }
            } else {
                "" // Single branch, no connection line
            };

            let mut branch_line = vec![
                Span::raw(branch_prefix),
                Span::styled(
                    &branch.name,
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                ),
            ];

            // Add "no commits" indicator if branch has no commits
            if branch.commits.is_empty() {
                branch_line.push(Span::raw(" "));
                branch_line.push(Span::styled(
                    "(no commits)",
                    Style::default().fg(Color::DarkGray),
                ));
            }

            items.push(ListItem::new(Line::from(branch_line)));

            // Add assigned files in branch
            for file in &branch.assignments {
                let file_prefix = if branch_count > 1 && !is_last_branch {
                    "│  " // Show vertical line if not the last branch
                } else if branch_count > 1 {
                    "   " // Last branch, no vertical line
                } else {
                    "  " // Single branch
                };

                let path = file.path.to_string();
                let mut spans = vec![
                    Span::raw(file_prefix),
                    Span::styled(path, Style::default().fg(Color::Yellow)),
                ];
                if let Some(lock_spans) = file_lock_spans(file) {
                    spans.push(Span::raw(" "));
                    spans.extend(lock_spans);
                }
                items.push(ListItem::new(Line::from(spans)));
            }

            // Add commits in branch
            for commit in &branch.commits {
                let prefix_no_dot = if branch_count > 1 && !is_last_branch {
                    "│  " // Show vertical line if not the last branch
                } else if branch_count > 1 {
                    "   " // Last branch, no vertical line
                } else {
                    "  " // Single branch
                };

                // Determine dot symbol and color based on commit state
                let (dot_symbol, dot_color) = match &commit.state {
                    but_workspace::ui::CommitState::LocalOnly => ("●", Color::White),
                    but_workspace::ui::CommitState::LocalAndRemote(object_id) => {
                        if object_id.to_string() == commit.full_id {
                            ("●", Color::Green)
                        } else {
                            ("◐", Color::Green)
                        }
                    }
                    but_workspace::ui::CommitState::Integrated => ("●", Color::Magenta),
                };

                let first_line = commit.message.lines().next().unwrap_or("").trim_end();

                let highlight = selected_lock_ids
                    .as_ref()
                    .map_or(false, |ids| ids.contains(&commit.full_id));
                let mut spans = vec![
                    Span::raw(prefix_no_dot.to_string()),
                    Span::styled(dot_symbol, Style::default().fg(dot_color)),
                    Span::raw(" ".to_string()),
                    Span::styled(commit.id.clone(), Style::default().fg(Color::Green)),
                    Span::raw(" ".to_string()),
                    Span::styled(first_line.to_string(), Style::default()),
                ];

                if highlight {
                    for span in &mut spans {
                        span.style = span.style.bg(Color::Yellow).add_modifier(Modifier::BOLD);
                    }
                }

                items.push(ListItem::new(Line::from(spans)));
            }
        }

        // Add blank line between stacks
        if !stack.branches.is_empty() {
            items.push(ListItem::new(Line::from("")));
        }
    }

    let total_items = app.count_status_items();
    let panel_num = if app.upstream_info.is_some() { 2 } else { 1 };
    let title = format!("Status ({} items) [{}]", total_items, panel_num);
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(border_style),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, area, &mut app.status_state);
}

fn render_oplog(f: &mut Frame, app: &mut LazyApp, area: Rect) {
    let is_active = matches!(app.active_panel, Panel::Oplog);
    let border_style = if is_active {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let items: Vec<ListItem> = app
        .oplog_entries
        .iter()
        .map(|entry| {
            let op_color = match entry.operation.as_str() {
                "CREATE" => Color::Green,
                "AMEND" | "REWORD" => Color::Yellow,
                "UNDO" | "RESTORE" => Color::Red,
                _ => Color::White,
            };

            ListItem::new(Line::from(vec![
                Span::styled(&entry.id, Style::default().fg(Color::Cyan)),
                Span::raw(" "),
                Span::styled(&entry.operation, Style::default().fg(op_color)),
                Span::raw(" "),
                Span::raw(&entry.title),
            ]))
        })
        .collect();

    let panel_num = if app.upstream_info.is_some() { 3 } else { 2 };
    let title = format!("Oplog ({}) [{}]", app.oplog_entries.len(), panel_num);
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(border_style),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    f.render_stateful_widget(list, area, &mut app.oplog_state);
}

fn render_main_view(f: &mut Frame, app: &LazyApp, area: Rect) {
    let title = app.get_details_title();

    // Apply border style based on selection
    let border_style = if app.details_selected {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let paragraph = Paragraph::new(app.main_view_content.clone())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(border_style),
        )
        .wrap(Wrap { trim: true })
        .scroll((app.details_scroll, 0));

    f.render_widget(paragraph, area);
}

fn render_command_log(f: &mut Frame, app: &LazyApp, area: Rect) {
    let log_lines: Vec<Line> = app
        .command_log
        .iter()
        .rev()
        .take(5)
        .rev()
        .map(|line| Line::from(line.clone()))
        .collect();

    let paragraph = Paragraph::new(log_lines)
        .block(Block::default().borders(Borders::ALL).title("Command Log"))
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

fn render_status_line(f: &mut Frame, app: &LazyApp, area: Rect) {
    let width = area.width as usize;
    let brand = "GitButler";
    let brand_len = brand.len();

    // Create the status line content with hints on the left and brand on the right
    let hints = if app.details_selected {
        "[h/l] scroll  •  [d] deselect  •  [@] toggle log  •  Press ? for help".to_string()
    } else if matches!(app.active_panel, Panel::Status) {
        let mut parts = vec![
            "[c] commit",
            "[e] edit",
            "[s] squash",
            "[u] uncommit",
            "[f] diff",
        ];
        if app.is_unassigned_header_selected() {
            parts.push("[a] absorb");
        }
        parts.push("[d] details");
        parts.push("[@] toggle log");
        parts.push("Press ? for help");
        parts.join("  •  ")
    } else if matches!(app.active_panel, Panel::Oplog) {
        if app.oplog_state.selected().is_some() {
            "[r] restore snapshot  •  [d] details  •  [@] toggle log  •  Press ? for help"
                .to_string()
        } else {
            "[d] select details  •  [@] toggle log  •  Press ? for help".to_string()
        }
    } else {
        "[d] select details  •  [@] toggle log  •  Press ? for help".to_string()
    };
    let hints_len = hints.len();

    // Calculate spacing to push brand to the right
    let spaces_needed = if width > hints_len + brand_len {
        width.saturating_sub(hints_len + brand_len)
    } else {
        0
    };

    let status_line = Line::from(vec![
        Span::styled(hints.clone(), Style::default().fg(Color::DarkGray)),
        Span::raw(" ".repeat(spaces_needed)),
        Span::styled(brand, Style::default().fg(Color::Cyan)),
    ]);

    let paragraph = Paragraph::new(status_line).style(Style::default().bg(Color::Black));

    f.render_widget(paragraph, area);
}

fn render_help_modal(f: &mut Frame, app: &LazyApp, area: Rect) {
    // Calculate centered modal area
    let modal_width = 60;
    let modal_height = 26;

    let modal_area = Rect {
        x: (area.width.saturating_sub(modal_width)) / 2,
        y: (area.height.saturating_sub(modal_height)) / 2,
        width: modal_width.min(area.width),
        height: modal_height.min(area.height),
    };

    // Clear the area
    f.render_widget(Clear, modal_area);

    // Create help text
    let help_text = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "GitButler Lazy TUI - Help",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Navigation",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  Tab / Shift+Tab      Switch between panels"),
        Line::from("  1 / 2                Jump to specific panel"),
        Line::from("  j / ↓                Move down in list"),
        Line::from("  k / ↑                Move up in list"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Panels",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  1: Upstream          Upstream commits"),
        Line::from("  2: Status            But Status"),
        Line::from("  3: Oplog             Operation history"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "General",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from("  c                    Open commit modal (Status panel)"),
        Line::from("  e                    Edit commit message (Status panel)"),
        Line::from("  u                    Uncommit selected commit (Status panel)"),
        Line::from("  f                    View commit diff (Status panel)"),
        Line::from("  r                    Refresh data"),
        Line::from("  r (Oplog)            Restore to selected snapshot"),
        Line::from("  @                    Hide/show command log"),
        Line::from("  f                    Fetch from remotes (Upstream panel)"),
        Line::from("  ?                    Toggle this help"),
        Line::from("  q / Esc              Quit (or close this help)"),
        Line::from("  Ctrl+C               Force quit"),
    ];

    let help_block = Paragraph::new(help_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Help (press ? or Esc to close) ")
                .title_alignment(Alignment::Center)
                .border_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.help_scroll, 0))
        .style(Style::default().bg(Color::Black));

    f.render_widget(help_block, modal_area);
}

fn render_commit_modal(f: &mut Frame, app: &LazyApp, area: Rect) {
    // Calculate centered modal area - larger to fit all fields
    let modal_width = 100;
    let modal_height = 35;

    let modal_area = Rect {
        x: (area.width.saturating_sub(modal_width)) / 2,
        y: (area.height.saturating_sub(modal_height)) / 2,
        width: modal_width.min(area.width),
        height: modal_height.min(area.height),
    };

    // Clear the area
    f.render_widget(Clear, modal_area);

    // Split into left (1/3) and right (2/3) sections
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(33), Constraint::Percentage(67)])
        .split(modal_area);

    // Left side: branch selection and files
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(main_chunks[0]);

    // Branch selection
    let branch_border = if matches!(app.commit_modal_focus, CommitModalFocus::BranchSelect) {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let branch_items: Vec<ListItem> = app
        .commit_branch_options
        .iter()
        .enumerate()
        .map(|(idx, opt)| {
            let symbol = if idx == app.commit_selected_branch_idx {
                "▶ "
            } else {
                "  "
            };
            let style = if idx == app.commit_selected_branch_idx {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if opt.is_new_branch {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(vec![
                Span::styled(symbol, style),
                Span::styled(&opt.branch_name, style),
            ]))
        })
        .collect();

    let branch_list = List::new(branch_items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(branch_border)
            .title(" Branch "),
    );

    f.render_widget(branch_list, left_chunks[0]);

    // Files to commit list
    let files_to_commit = render_files_to_commit_list(app);
    let files_border = if matches!(app.commit_modal_focus, CommitModalFocus::Files) {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    let files_list = List::new(files_to_commit).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(files_border)
            .title(format!(
                " Files to Commit {} ",
                if app.commit_only_mode {
                    "[Only Mode]"
                } else {
                    "[All]"
                }
            )),
    );

    f.render_widget(files_list, left_chunks[1]);

    // Only mode toggle hint
    let only_hint = vec![Line::from(vec![
        Span::styled(
            "Ctrl+O",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(": toggle only mode"),
    ])];
    let only_block = Paragraph::new(only_hint)
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().bg(Color::Black));
    f.render_widget(only_block, left_chunks[2]);

    // Right side: subject, message, and hints
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(main_chunks[1]);

    // Subject field
    let subject_border = if matches!(app.commit_modal_focus, CommitModalFocus::Subject) {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let subject_block = Paragraph::new(app.commit_subject.clone())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(subject_border)
                .title(" Subject "),
        )
        .style(Style::default().bg(Color::Black));

    f.render_widget(subject_block, right_chunks[0]);

    // Message field
    let message_border = if matches!(app.commit_modal_focus, CommitModalFocus::Message) {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let message_block = Paragraph::new(app.commit_message.clone())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(message_border)
                .title(" Message (optional) "),
        )
        .wrap(Wrap { trim: false })
        .style(Style::default().bg(Color::Black));

    f.render_widget(message_block, right_chunks[1]);

    // Hints at bottom
    let hints = vec![Line::from(vec![
        Span::styled(
            "Tab",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(": next field  •  "),
        Span::styled(
            "j/k",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(": move  •  "),
        Span::styled(
            "Space",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(": toggle file  •  "),
        Span::styled(
            "Ctrl+M",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(": commit  •  "),
        Span::styled(
            "Esc",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw(": cancel"),
    ])];

    let hints_block = Paragraph::new(hints)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .style(Style::default().bg(Color::Black));

    f.render_widget(hints_block, right_chunks[2]);
}

fn render_update_modal(f: &mut Frame, app: &LazyApp, area: Rect) {
    let modal_width = 60;
    let modal_height = 8;
    let modal_area = Rect {
        x: (area.width.saturating_sub(modal_width)) / 2,
        y: (area.height.saturating_sub(modal_height)) / 2,
        width: modal_width.min(area.width),
        height: modal_height.min(area.height),
    };

    f.render_widget(Clear, modal_area);

    let status_line = match &app.upstream_integration_status {
        Some(StackStatuses::UpToDate) => "All applied branches are already up to date.".to_string(),
        Some(StackStatuses::UpdatesRequired { statuses, .. }) => {
            let branch_count: usize = statuses
                .iter()
                .map(|(_, stack)| stack.branch_statuses.len())
                .sum();
            if branch_count == 0 {
                "No active branches require updates.".to_string()
            } else {
                format!("{} branch(es) will be rebased onto upstream.", branch_count)
            }
        }
        None => "Branch status unknown; attempting rebase.".to_string(),
    };

    let content = vec![
        Line::from(vec![Span::styled(
            "Rebase applied branches onto upstream?",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(status_line),
        Line::from(""),
        Line::from("Press Enter/‘y’ to confirm or Esc/‘n’ to cancel."),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title("Confirm Update")
        .style(Style::default().bg(Color::Black));

    f.render_widget(
        Paragraph::new(content)
            .block(block)
            .alignment(Alignment::Center),
        modal_area,
    );
}

fn render_restore_modal(f: &mut Frame, app: &LazyApp, area: Rect) {
    let modal_width = 70;
    let modal_height = 9;
    let modal_area = Rect {
        x: (area.width.saturating_sub(modal_width)) / 2,
        y: (area.height.saturating_sub(modal_height)) / 2,
        width: modal_width.min(area.width),
        height: modal_height.min(area.height),
    };

    f.render_widget(Clear, modal_area);

    let mut content = vec![Line::from(vec![Span::styled(
        "Restore workspace to selected snapshot?",
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )])];

    if let Some(target) = &app.restore_target {
        content.push(Line::from(""));
        content.push(Line::from(vec![
            Span::styled(
                format!("Snapshot {}", target.id),
                Style::default().fg(Color::Cyan),
            ),
            Span::raw(": "),
            Span::raw(&target.title),
        ]));
        content.push(Line::from(vec![
            Span::styled("Time: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(&target.time),
        ]));
    }

    content.push(Line::from(""));
    content.push(Line::from(vec![Span::styled(
        "This will overwrite any uncommitted work in your workspace.",
        Style::default().fg(Color::Red),
    )]));
    content.push(Line::from(
        "Press Enter/‘y’ to confirm or Esc/‘n’ to cancel.",
    ));

    let block = Block::default()
        .borders(Borders::ALL)
        .title("Confirm Restore")
        .style(Style::default().bg(Color::Black));

    f.render_widget(
        Paragraph::new(content)
            .block(block)
            .alignment(Alignment::Left),
        modal_area,
    );
}

fn render_reword_modal(f: &mut Frame, app: &LazyApp, area: Rect) {
    let modal_width = 90;
    let modal_height = 25;

    let modal_area = Rect {
        x: (area.width.saturating_sub(modal_width)) / 2,
        y: (area.height.saturating_sub(modal_height)) / 2,
        width: modal_width.min(area.width),
        height: modal_height.min(area.height),
    };

    f.render_widget(Clear, modal_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(modal_area);

    let mut info_lines = Vec::new();
    if let Some(target) = &app.reword_target {
        info_lines.push(Line::from(vec![
            Span::styled(
                "Editing commit ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                target.commit_short_id.clone(),
                Style::default().fg(Color::Green),
            ),
            Span::raw(" on "),
            Span::styled(target.branch_name.clone(), Style::default().fg(Color::Blue)),
        ]));
        info_lines.push(Line::from(vec![
            Span::styled("Stack:", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!(" {:?}", target.stack_id)),
        ]));
    } else {
        info_lines.push(Line::from("No commit selected"));
    }
    let info_block = Paragraph::new(info_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Edit Commit Message ")
                .border_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
        )
        .wrap(Wrap { trim: true })
        .style(Style::default().bg(Color::Black));
    f.render_widget(info_block, chunks[0]);

    let subject_border = if matches!(app.reword_modal_focus, RewordModalFocus::Subject) {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let subject_block = Paragraph::new(app.reword_subject.clone())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(subject_border)
                .title(" Subject "),
        )
        .style(Style::default().bg(Color::Black));
    f.render_widget(subject_block, chunks[1]);

    let message_border = if matches!(app.reword_modal_focus, RewordModalFocus::Message) {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let message_block = Paragraph::new(app.reword_message.clone())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(message_border)
                .title(" Body "),
        )
        .wrap(Wrap { trim: false })
        .style(Style::default().bg(Color::Black));
    f.render_widget(message_block, chunks[2]);

    let hints = vec![Line::from(vec![
        Span::styled(
            "Tab",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(": next field  •  "),
        Span::styled(
            "Ctrl+M",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(": save & rebase  •  "),
        Span::styled(
            "Esc",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw(": cancel"),
    ])];
    let hints_block = Paragraph::new(hints)
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().bg(Color::Black));
    f.render_widget(hints_block, chunks[3]);
}

fn render_uncommit_modal(f: &mut Frame, app: &LazyApp, area: Rect) {
    let modal_width = 70;
    let modal_height = 12;

    let modal_area = Rect {
        x: (area.width.saturating_sub(modal_width)) / 2,
        y: (area.height.saturating_sub(modal_height)) / 2,
        width: modal_width.min(area.width),
        height: modal_height.min(area.height),
    };

    f.render_widget(Clear, modal_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(3),
            Constraint::Length(3),
        ])
        .split(modal_area);

    let mut lines = Vec::new();
    if let Some(target) = &app.uncommit_target {
        lines.push(Line::from(vec![
            Span::styled(
                "Uncommit commit ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                target.commit_short_id.clone(),
                Style::default().fg(Color::Green),
            ),
            Span::raw(" from "),
            Span::styled(target.branch_name.clone(), Style::default().fg(Color::Blue)),
            Span::raw("?"),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            target.commit_message.lines().next().unwrap_or(""),
            Style::default().fg(Color::Yellow),
        )]));
    } else {
        lines.push(Line::from("No commit selected"));
    }

    let info_block = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Confirm Uncommit ")
                .border_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        )
        .wrap(Wrap { trim: true })
        .style(Style::default().bg(Color::Black));
    f.render_widget(info_block, chunks[0]);

    let warning = Paragraph::new(vec![Line::from(
        "This moves the commit's changes back to the worktree.",
    )])
    .block(Block::default().borders(Borders::ALL))
    .wrap(Wrap { trim: true })
    .style(Style::default().bg(Color::Black));
    f.render_widget(warning, chunks[1]);

    let hints = vec![Line::from(vec![
        Span::styled(
            "Enter/Y",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(": uncommit  •  "),
        Span::styled(
            "Esc/N",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw(": cancel"),
    ])];
    let hints_block = Paragraph::new(hints)
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().bg(Color::Black));
    f.render_widget(hints_block, chunks[2]);
}

fn render_absorb_modal(f: &mut Frame, app: &LazyApp, area: Rect) {
    let modal_width = 70;
    let modal_height = 12;

    let modal_area = Rect {
        x: (area.width.saturating_sub(modal_width)) / 2,
        y: (area.height.saturating_sub(modal_height)) / 2,
        width: modal_width.min(area.width),
        height: modal_height.min(area.height),
    };

    f.render_widget(Clear, modal_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(3),
            Constraint::Length(3),
        ])
        .split(modal_area);

    let mut lines = Vec::new();
    if let Some(summary) = &app.absorb_summary {
        lines.push(Line::from(vec![Span::styled(
            format!("Absorb {} unassigned file(s)?", summary.file_count),
            Style::default().add_modifier(Modifier::BOLD),
        )]));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            format!("Hunks: {}", summary.hunk_count),
            Style::default().fg(Color::Cyan),
        )]));
        lines.push(Line::from(vec![
            Span::styled(
                format!("+{}", summary.total_additions),
                Style::default().fg(Color::Green),
            ),
            Span::raw("  "),
            Span::styled(
                format!("-{}", summary.total_removals),
                Style::default().fg(Color::Red),
            ),
        ]));
    } else {
        lines.push(Line::from("No unassigned files available"));
    }

    let info_block = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Confirm Absorb ")
                .border_style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
        )
        .wrap(Wrap { trim: true })
        .style(Style::default().bg(Color::Black));
    f.render_widget(info_block, chunks[0]);

    let warning = Paragraph::new(vec![Line::from(
        "Absorb amends these changes into their target commits.",
    )])
    .block(Block::default().borders(Borders::ALL))
    .wrap(Wrap { trim: true })
    .style(Style::default().bg(Color::Black));
    f.render_widget(warning, chunks[1]);

    let hints = vec![Line::from(vec![
        Span::styled(
            "Enter/Y",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(": absorb  •  "),
        Span::styled(
            "Esc/N",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw(": cancel"),
    ])];
    let hints_block = Paragraph::new(hints)
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().bg(Color::Black));
    f.render_widget(hints_block, chunks[2]);
}

fn render_squash_modal(f: &mut Frame, app: &LazyApp, area: Rect) {
    let modal_width = 80;
    let modal_height = 14;

    let modal_area = Rect {
        x: (area.width.saturating_sub(modal_width)) / 2,
        y: (area.height.saturating_sub(modal_height)) / 2,
        width: modal_width.min(area.width),
        height: modal_height.min(area.height),
    };

    f.render_widget(Clear, modal_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Min(3),
            Constraint::Length(3),
        ])
        .split(modal_area);

    let mut lines = Vec::new();
    if let Some(target) = &app.squash_target {
        lines.push(Line::from(vec![
            Span::styled("Squash ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                target.source_short_id.clone(),
                Style::default().fg(Color::Green),
            ),
            Span::raw(" into "),
            Span::styled(
                target.destination_short_id.clone(),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw(" on "),
            Span::styled(target.branch_name.clone(), Style::default().fg(Color::Blue)),
            Span::raw("?"),
        ]));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Source:", Style::default().fg(Color::DarkGray)),
            Span::raw(" "),
            Span::styled(
                target.source_message.lines().next().unwrap_or(""),
                Style::default().fg(Color::Green),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Into:", Style::default().fg(Color::DarkGray)),
            Span::raw("  "),
            Span::styled(
                target.destination_message.lines().next().unwrap_or(""),
                Style::default().fg(Color::Yellow),
            ),
        ]));
    } else {
        lines.push(Line::from("No commits selected"));
    }

    let info_block = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Confirm Squash ")
                .border_style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
        )
        .wrap(Wrap { trim: true })
        .style(Style::default().bg(Color::Black));
    f.render_widget(info_block, chunks[0]);

    let warning = Paragraph::new(vec![Line::from(
        "Combines both commits and rewrites stack history.",
    )])
    .block(Block::default().borders(Borders::ALL))
    .wrap(Wrap { trim: true })
    .style(Style::default().bg(Color::Black));
    f.render_widget(warning, chunks[1]);

    let hints = vec![Line::from(vec![
        Span::styled(
            "Enter/Y",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(": squash  •  "),
        Span::styled(
            "Esc/N",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw(": cancel"),
    ])];
    let hints_block = Paragraph::new(hints)
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().bg(Color::Black));
    f.render_widget(hints_block, chunks[2]);
}

fn render_diff_modal(f: &mut Frame, app: &LazyApp, area: Rect) {
    let modal_width = 110;
    let modal_height = 38;

    let modal_area = Rect {
        x: (area.width.saturating_sub(modal_width)) / 2,
        y: (area.height.saturating_sub(modal_height)) / 2,
        width: modal_width.min(area.width),
        height: modal_height.min(area.height),
    };

    f.render_widget(Clear, modal_area);

    let vertical_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(3)])
        .split(modal_area);

    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(30), Constraint::Min(10)])
        .split(vertical_chunks[0]);

    // Files sidebar
    let selected_idx = if app.diff_modal_files.is_empty() {
        0
    } else {
        app.diff_modal_selected_file
            .min(app.diff_modal_files.len().saturating_sub(1))
    };

    let file_items: Vec<ListItem> = if app.diff_modal_files.is_empty() {
        vec![ListItem::new(Line::from("No files"))]
    } else {
        app.diff_modal_files
            .iter()
            .enumerate()
            .map(|(idx, file)| {
                let is_selected = idx == selected_idx;
                let symbol = if is_selected { "▶" } else { " " };
                let style = if is_selected {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                let status_char = match &file.status {
                    but_core::ui::TreeStatus::Addition { .. } => 'A',
                    but_core::ui::TreeStatus::Deletion { .. } => 'D',
                    but_core::ui::TreeStatus::Modification { .. } => 'M',
                    but_core::ui::TreeStatus::Rename { .. } => 'R',
                };
                ListItem::new(Line::from(vec![
                    Span::styled(format!("{} {} ", symbol, status_char), style),
                    Span::styled(file.path.clone(), style),
                ]))
            })
            .collect()
    };

    let files_list = List::new(file_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Files ")
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(files_list, content_chunks[0]);

    // Diff viewer
    let diff_lines = if app.diff_modal_files.is_empty() {
        vec![Line::from("Select a file to view its diff")]
    } else {
        render_diff_lines(&app.diff_modal_files[selected_idx])
    };

    let diff_title = if let Some(file) = app.diff_modal_files.get(selected_idx) {
        format!(" Diff: {} ", file.path)
    } else {
        " Diff ".to_string()
    };

    let diff_split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(5), Constraint::Length(2)])
        .split(content_chunks[1]);

    let viewport_height = diff_split[0].height.saturating_sub(2) as usize;
    let total_lines = diff_lines.len();
    let max_scroll = total_lines.saturating_sub(viewport_height.max(1));
    let scroll = app.diff_modal_scroll.min(max_scroll as u16);

    let diff_block = Paragraph::new(diff_lines.clone())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(diff_title)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    f.render_widget(diff_block, diff_split[0]);

    render_scroll_indicator(
        f,
        diff_split[1],
        total_lines,
        viewport_height,
        scroll as usize,
        max_scroll,
    );

    // Hints area
    let hints = vec![Line::from(vec![
        Span::styled(
            "j/k",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(": scroll  •  "),
        Span::styled(
            "h/l",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(": change file  •  "),
        Span::styled(
            "[/]",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(": prev/next hunk  •  "),
        Span::styled(
            "Esc",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw(": close"),
    ])];

    let hints_block = Paragraph::new(hints)
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().bg(Color::Black));
    f.render_widget(hints_block, vertical_chunks[1]);
}

fn render_branch_rename_modal(f: &mut Frame, app: &LazyApp, area: Rect) {
    let modal_width = 70;
    let modal_height = 10;

    let modal_area = Rect {
        x: (area.width.saturating_sub(modal_width)) / 2,
        y: (area.height.saturating_sub(modal_height)) / 2,
        width: modal_width.min(area.width),
        height: modal_height.min(area.height),
    };

    f.render_widget(Clear, modal_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(modal_area);

    let title = if let Some(target) = &app.branch_rename_target {
        format!("Rename branch '{}'", target.current_name)
    } else {
        "Rename Branch".to_string()
    };

    let input_block = Paragraph::new(app.branch_rename_input.clone())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .style(Style::default().bg(Color::Black));
    f.render_widget(input_block, chunks[0]);

    let instructions = Paragraph::new(vec![Line::from(
        "Enter a new branch name. Existing branch history will be preserved.",
    )])
    .block(Block::default().borders(Borders::ALL))
    .wrap(Wrap { trim: true })
    .style(Style::default().bg(Color::Black));
    f.render_widget(instructions, chunks[1]);

    let hints = vec![Line::from(vec![
        Span::styled(
            "Enter",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(": rename  •  "),
        Span::styled(
            "Ctrl+M",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(": rename  •  "),
        Span::styled(
            "Esc",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw(": cancel"),
    ])];

    let hints_block = Paragraph::new(hints)
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().bg(Color::Black));
    f.render_widget(hints_block, chunks[2]);
}

fn render_scroll_indicator(
    f: &mut Frame,
    area: Rect,
    total_lines: usize,
    viewport_height: usize,
    scroll: usize,
    max_scroll: usize,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    if total_lines == 0 || viewport_height == 0 || total_lines <= viewport_height {
        let block =
            Paragraph::new(vec![Line::from("  ")]).block(Block::default().borders(Borders::NONE));
        f.render_widget(block, area);
        return;
    }

    let slider_style = Style::default().fg(Color::DarkGray);
    let height = area.height as usize;

    let track_height = height.saturating_sub(2);
    let thumb_height = cmp::max(
        1,
        ((viewport_height as f32 / total_lines as f32) * track_height.max(1) as f32).round()
            as usize,
    );
    let max_thumb_pos = track_height.saturating_sub(thumb_height);
    let thumb_pos = if max_scroll == 0 {
        0
    } else {
        ((scroll as f32 / max_scroll as f32) * max_thumb_pos as f32)
            .round()
            .clamp(0.0, max_thumb_pos as f32) as usize
    };

    let mut lines = Vec::with_capacity(height);
    if height > 0 {
        lines.push(Line::from(vec![Span::styled("^", slider_style)]));
    }

    for i in 0..track_height {
        let symbol = if i >= thumb_pos && i < thumb_pos + thumb_height {
            "#"
        } else {
            "|"
        };
        lines.push(Line::from(vec![Span::styled(symbol, slider_style)]));
    }

    if height > 1 {
        lines.push(Line::from(vec![Span::styled("v", slider_style)]));
    }

    let indicator = Paragraph::new(lines)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(indicator, area);
}

fn render_diff_lines(file: &super::app::CommitDiffFile) -> Vec<Line<'static>> {
    if file.lines.is_empty() {
        return vec![Line::from("No diff available")];
    }

    file.lines
        .iter()
        .map(|line| {
            let style = match line.kind {
                DiffLineKind::Header => Style::default().fg(Color::Cyan),
                DiffLineKind::Added => Style::default().fg(Color::Green),
                DiffLineKind::Removed => Style::default().fg(Color::Red),
                DiffLineKind::Info => Style::default().fg(Color::Yellow),
                DiffLineKind::Context => Style::default(),
            };
            Line::from(vec![Span::styled(line.text.clone(), style)])
        })
        .collect()
}

fn render_files_to_commit_list(app: &LazyApp) -> Vec<ListItem<'_>> {
    if app.commit_files.is_empty() {
        return vec![ListItem::new(Line::from(vec![Span::styled(
            "  No files to commit",
            Style::default().fg(Color::DarkGray),
        )]))];
    }

    app.commit_files
        .iter()
        .enumerate()
        .map(|(idx, entry)| {
            let key = entry.file.path.to_str_lossy().into_owned();
            let is_selected = app.commit_selected_file_paths.contains(&key);
            let checkbox = if is_selected { "[x]" } else { "[ ]" };
            let is_cursor = matches!(app.commit_modal_focus, CommitModalFocus::Files)
                && idx == app.commit_selected_file_idx;

            let mut spans = vec![
                Span::styled(
                    if is_cursor { "›" } else { " " },
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw(" "),
                Span::styled(checkbox, Style::default()),
                Span::raw(" "),
                Span::styled(key.clone(), Style::default().fg(Color::Yellow)),
            ];

            if let Some(lock_spans) = file_lock_spans(&entry.file) {
                spans.push(Span::raw(" "));
                spans.extend(lock_spans);
            }

            if is_cursor {
                spans = spans
                    .into_iter()
                    .map(|mut span| {
                        span.style = span
                            .style
                            .bg(Color::Blue)
                            .fg(Color::Black)
                            .add_modifier(Modifier::BOLD);
                        span
                    })
                    .collect();
            }

            ListItem::new(Line::from(spans))
        })
        .collect()
}

fn file_lock_spans(file: &FileAssignment) -> Option<Vec<Span<'static>>> {
    let locks = collect_lock_ids(file);
    if locks.is_empty() {
        return None;
    }

    let mut spans = Vec::new();
    spans.push(Span::styled(
        "🔒",
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD),
    ));
    spans.push(Span::raw(" "));

    let mut first = true;
    for commit_id in locks {
        if commit_id.len() < 7 {
            continue;
        }
        if !first {
            spans.push(Span::raw(", "));
        }
        first = false;
        let (prefix, rest) = commit_id.split_at(2);
        let short_rest = &rest[..5.min(rest.len())];
        spans.push(Span::styled(
            prefix.to_string(),
            Style::default()
                .fg(Color::LightBlue)
                .add_modifier(Modifier::UNDERLINED),
        ));
        spans.push(Span::styled(
            short_rest.to_string(),
            Style::default().fg(Color::LightBlue),
        ));
    }

    if spans.len() <= 2 { None } else { Some(spans) }
}

fn collect_lock_ids(file: &FileAssignment) -> BTreeSet<String> {
    let mut locks = BTreeSet::new();
    for assignment in &file.assignments {
        if let Some(hunk_locks) = &assignment.inner.hunk_locks {
            for lock in hunk_locks {
                locks.insert(lock.commit_id.to_string());
            }
        }
    }
    locks
}

fn selected_lock_ids(app: &LazyApp) -> Option<BTreeSet<String>> {
    app.get_selected_file()
        .map(collect_lock_ids)
        .filter(|locks| !locks.is_empty())
}

fn branch_status_summary(status: &UpstreamBranchStatus) -> (&'static str, &'static str, Style) {
    match status {
        UpstreamBranchStatus::SaflyUpdatable => {
            ("✅", "Updatable", Style::default().fg(Color::Green))
        }
        UpstreamBranchStatus::Integrated => ("🔄", "Integrated", Style::default().fg(Color::Blue)),
        UpstreamBranchStatus::Conflicted { rebasable } => {
            if *rebasable {
                (
                    "⚠️",
                    "Conflicted (rebasable)",
                    Style::default().fg(Color::Yellow),
                )
            } else {
                (
                    "❗️",
                    "Conflicted",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )
            }
        }
        UpstreamBranchStatus::Empty => {
            ("⬜", "Nothing to do", Style::default().fg(Color::DarkGray))
        }
    }
}

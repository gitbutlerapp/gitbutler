use bstr::ByteSlice;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Clear, List, ListItem, Paragraph, Wrap,
};
use ratatui::Frame;

use super::app::{App, CommitModalFocus, Panel, StatusItem};

// ---------------------------------------------------------------------------
// Main UI entry point
// ---------------------------------------------------------------------------

pub(super) fn ui(f: &mut Frame, app: &App) {
    let size = f.area();

    // Top-level vertical split: main area + command log + help bar
    let log_height = if app.command_log_visible { 6 } else { 0 };
    let outer_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(10),
            Constraint::Length(log_height),
            Constraint::Length(1),
        ])
        .split(size);

    let main_area = outer_chunks[0];
    let log_area = outer_chunks[1];
    let helpbar_area = outer_chunks[2];

    // Horizontal split: left panels (40%) + details (60%)
    let h_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(main_area);

    // Left side: vertical split into upstream + status + oplog
    let has_upstream = app.upstream_info.is_some();
    let left_chunks = if has_upstream {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(8),
                Constraint::Length(10),
            ])
            .split(h_chunks[0])
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(0),
                Constraint::Min(8),
                Constraint::Length(10),
            ])
            .split(h_chunks[0])
    };

    // Render panels
    render_upstream_panel(f, app, left_chunks[0]);
    render_status_panel(f, app, left_chunks[1]);
    render_oplog_panel(f, app, left_chunks[2]);
    render_details_panel(f, app, h_chunks[1]);

    // Command log
    if app.command_log_visible {
        render_command_log(f, app, log_area);
    }

    // Help bar
    render_help_bar(f, app, helpbar_area);

    // Overlays
    if app.show_commit_modal {
        render_commit_modal(f, app, size);
    }
    if app.show_help {
        render_help_overlay(f, app, size);
    }
}

// ---------------------------------------------------------------------------
// Upstream panel
// ---------------------------------------------------------------------------

fn render_upstream_panel(f: &mut Frame, app: &App, area: Rect) {
    // Store area for mouse detection (we need to cast away mutability concern)
    // This is handled by storing in the app before render in the main loop
    let is_active = app.active_panel == Panel::Upstream;
    let border_style = panel_border_style(is_active);

    let content = if let Some(info) = &app.upstream_info {
        let fetched = info
            .last_fetched_ms
            .map(|ms| format_relative_time(ms))
            .unwrap_or_else(|| "never".to_string());

        Line::from(vec![
            Span::styled(
                format!(" {} behind", info.behind_count),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw("  "),
            Span::styled(
                format!("fetched {fetched}"),
                Style::default().fg(Color::DarkGray),
            ),
        ])
    } else {
        Line::from(Span::styled(
            " Up to date",
            Style::default().fg(Color::Green),
        ))
    };

    let block = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Upstream ")
            .border_style(border_style),
    );
    f.render_widget(block, area);
}

// ---------------------------------------------------------------------------
// Status panel
// ---------------------------------------------------------------------------

fn render_status_panel(f: &mut Frame, app: &App, area: Rect) {
    let is_active = app.active_panel == Panel::Status && !app.details_selected;
    let border_style = panel_border_style(is_active);

    let mut items: Vec<ListItem> = Vec::new();

    // Unassigned files
    if !app.unassigned_files.is_empty() {
        items.push(ListItem::new(Line::from(vec![
            Span::styled("╭┄", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "unstaged changes",
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
        ])));

        for file in &app.unassigned_files {
            let path = file.path.to_str_lossy();
            items.push(ListItem::new(Line::from(vec![
                Span::styled("┊   ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    path.into_owned(),
                    Style::default().fg(Color::Yellow),
                ),
            ])));
        }

        items.push(ListItem::new(Line::from(vec![
            Span::styled("┊", Style::default().fg(Color::DarkGray)),
        ])));
    }

    // Stacks & branches
    for (si, stack) in app.stacks.iter().enumerate() {
        let has_staged = stack
            .branches
            .first()
            .map_or(false, |b| !b.files.is_empty());
        let first_branch_name = stack
            .branches
            .first()
            .map(|b| b.name.clone())
            .unwrap_or_default();

        // Staged files header (shown above the branch)
        if has_staged {
            items.push(ListItem::new(Line::from(vec![
                Span::styled("╭┄", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("staged to {first_branch_name}"),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            ])));

            for file in &stack.branches[0].files {
                let path = file.path.to_str_lossy();
                items.push(ListItem::new(Line::from(vec![
                    Span::styled("┊   ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        path.into_owned(),
                        Style::default().fg(Color::Yellow),
                    ),
                ])));
            }
        }

        for (bi, branch) in stack.branches.iter().enumerate() {
            let is_first_visual = bi == 0 && !has_staged;

            let no_commits_hint = if branch.commits.is_empty() {
                " (no commits)"
            } else {
                ""
            };

            // Branch header: first visual item uses ╭┄, subsequent use ├┄
            let prefix = if is_first_visual { "╭┄" } else { "├┄" };
            items.push(ListItem::new(Line::from(vec![
                Span::styled(
                    prefix,
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    branch.name.clone(),
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    no_commits_hint,
                    Style::default().fg(Color::DarkGray),
                ),
            ])));

            // Assigned files (skip first branch files if shown in staged section)
            if !(bi == 0 && has_staged) {
                for file in &branch.files {
                    let path = file.path.to_str_lossy();
                    items.push(ListItem::new(Line::from(vec![
                        Span::styled("┊   ", Style::default().fg(Color::DarkGray)),
                        Span::styled(
                            path.into_owned(),
                            Style::default().fg(Color::Yellow),
                        ),
                    ])));
                }
            }

            // Commits
            for commit in &branch.commits {
                let dot = commit_status_dot(&commit.state);
                let msg = commit.message.lines().next().unwrap_or("");
                items.push(ListItem::new(Line::from(vec![
                    Span::styled("┊", Style::default().fg(Color::DarkGray)),
                    dot,
                    Span::raw("   "),
                    Span::styled(
                        commit.id.clone(),
                        Style::default().fg(Color::Green),
                    ),
                    Span::raw(" "),
                    Span::raw(msg.to_string()),
                ])));
            }
        }

        // Stack footer
        if !stack.branches.is_empty() {
            items.push(ListItem::new(Line::from(vec![
                Span::styled("╰╯", Style::default().fg(Color::DarkGray)),
            ])));
        }

        // Separator between stacks
        if si + 1 < app.stacks.len() {
            items.push(ListItem::new(Line::from("")));
        }
    }

    if items.is_empty() {
        items.push(ListItem::new(Line::from(Span::styled(
            "  No branches or files",
            Style::default().fg(Color::DarkGray),
        ))));
    }

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Status ")
                .border_style(border_style),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

    f.render_stateful_widget(list, area, &mut app.status_state.clone());
}

// ---------------------------------------------------------------------------
// Oplog panel
// ---------------------------------------------------------------------------

fn render_oplog_panel(f: &mut Frame, app: &App, area: Rect) {
    let is_active = app.active_panel == Panel::Oplog;
    let border_style = panel_border_style(is_active);

    let items: Vec<ListItem> = app
        .oplog_entries
        .iter()
        .map(|entry| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    entry.id.clone(),
                    Style::default().fg(Color::Green),
                ),
                Span::raw(" "),
                Span::styled(
                    format!("{:<8}", entry.operation),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw(" "),
                Span::raw(entry.title.clone()),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Oplog ")
                .border_style(border_style),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

    f.render_stateful_widget(list, area, &mut app.oplog_state.clone());
}

// ---------------------------------------------------------------------------
// Details panel
// ---------------------------------------------------------------------------

fn render_details_panel(f: &mut Frame, app: &App, area: Rect) {
    let is_active = app.details_selected;
    let border_style = panel_border_style(is_active);

    let lines = build_details_content(app);

    let block = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Details ")
                .border_style(border_style),
        )
        .wrap(Wrap { trim: false })
        .scroll((app.details_scroll, 0));

    f.render_widget(block, area);
}

fn build_details_content(app: &App) -> Vec<Line<'static>> {
    match app.active_panel {
        Panel::Status => build_status_details(app),
        Panel::Oplog => build_oplog_details(app),
        Panel::Upstream => build_upstream_details(app),
    }
}

fn build_status_details(app: &App) -> Vec<Line<'static>> {
    let item = match app.selected_status_item() {
        Some(item) => item,
        None => {
            return vec![Line::from(Span::styled(
                "Select an item to see details",
                Style::default().fg(Color::DarkGray),
            ))];
        }
    };

    match item {
        StatusItem::UnassignedHeader => {
            let count = app.unassigned_files.len();
            vec![
                Line::from(vec![Span::styled(
                    "Unassigned Files",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )]),
                Line::from(""),
                Line::from(format!("{count} file(s) not assigned to any branch.")),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "Use 'but stage <file> <branch>' to assign, or 'but absorb' to auto-assign.",
                    Style::default().fg(Color::DarkGray),
                )]),
            ]
        }
        StatusItem::UnassignedFile(idx) => {
            if let Some(file) = app.unassigned_files.get(idx) {
                let path = file.path.to_str_lossy().into_owned();
                vec![
                    Line::from(vec![
                        Span::styled("File: ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::styled(path, Style::default().fg(Color::Yellow)),
                    ]),
                    Line::from(""),
                    Line::from(format!("{} hunk(s)", file.assignments.len())),
                ]
            } else {
                vec![]
            }
        }
        StatusItem::StagedHeader { stack } => {
            if let Some(s) = app.stacks.get(stack) {
                let branch_name = s
                    .branches
                    .first()
                    .map(|b| b.name.clone())
                    .unwrap_or_default();
                let file_count = s
                    .branches
                    .first()
                    .map(|b| b.files.len())
                    .unwrap_or(0);
                vec![
                    Line::from(vec![
                        Span::styled(
                            "Staged to: ",
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(branch_name, Style::default().fg(Color::Blue)),
                    ]),
                    Line::from(""),
                    Line::from(format!("{file_count} file(s) staged for next commit.")),
                ]
            } else {
                vec![]
            }
        }
        StatusItem::Branch { stack, branch } => {
            if let Some(b) = app.stacks.get(stack).and_then(|s| s.branches.get(branch)) {
                let commit_count = b.commits.len();
                let file_count = b.files.len();
                vec![
                    Line::from(vec![
                        Span::styled("Branch: ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::styled(b.name.clone(), Style::default().fg(Color::Blue)),
                    ]),
                    Line::from(""),
                    Line::from(format!("{commit_count} commit(s), {file_count} staged file(s)")),
                ]
            } else {
                vec![]
            }
        }
        StatusItem::AssignedFile { stack, branch, file } => {
            if let Some(f) = app
                .stacks
                .get(stack)
                .and_then(|s| s.branches.get(branch))
                .and_then(|b| b.files.get(file))
            {
                let path = f.path.to_str_lossy().into_owned();
                vec![
                    Line::from(vec![
                        Span::styled("File: ", Style::default().add_modifier(Modifier::BOLD)),
                        Span::styled(path, Style::default().fg(Color::Yellow)),
                    ]),
                    Line::from(""),
                    Line::from(format!(
                        "Staged to branch, {} hunk(s)",
                        f.assignments.len()
                    )),
                ]
            } else {
                vec![]
            }
        }
        StatusItem::Commit {
            stack,
            branch,
            commit,
        } => {
            if let Some(c) = app
                .stacks
                .get(stack)
                .and_then(|s| s.branches.get(branch))
                .and_then(|b| b.commits.get(commit))
            {
                let branch_name = app
                    .stacks
                    .get(stack)
                    .and_then(|s| s.branches.get(branch))
                    .map(|b| b.name.clone())
                    .unwrap_or_default();

                let mut lines = vec![
                    Line::from(vec![
                        Span::styled(
                            "Commit ",
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(c.id.clone(), Style::default().fg(Color::Green)),
                        Span::raw(" on "),
                        Span::styled(branch_name, Style::default().fg(Color::Blue)),
                    ]),
                    Line::from(vec![
                        Span::styled(
                            "SHA:    ",
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(c.full_id.clone(), Style::default().fg(Color::DarkGray)),
                    ]),
                    Line::from(""),
                ];

                // Full commit message
                for line in c.message.lines() {
                    lines.push(Line::from(line.to_string()));
                }

                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::styled(
                        "Author: ",
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(c.author.clone()),
                ]));
                lines.push(Line::from(vec![
                    Span::styled("Date:   ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(c.created_at.clone()),
                ]));

                // State
                let state_desc = match &c.state {
                    but_workspace::ui::CommitState::LocalOnly => "Local only",
                    but_workspace::ui::CommitState::LocalAndRemote(_) => "Pushed to remote",
                    but_workspace::ui::CommitState::Integrated => "Integrated",
                };
                lines.push(Line::from(vec![
                    Span::styled(
                        "State:  ",
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(state_desc.to_string()),
                ]));

                lines
            } else {
                vec![]
            }
        }
    }
}

fn build_oplog_details(app: &App) -> Vec<Line<'static>> {
    let idx = match app.oplog_state.selected() {
        Some(idx) => idx,
        None => {
            return vec![Line::from(Span::styled(
                "Select an oplog entry",
                Style::default().fg(Color::DarkGray),
            ))];
        }
    };

    if let Some(entry) = app.oplog_entries.get(idx) {
        vec![
            Line::from(vec![
                Span::styled(
                    "Operation: ",
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(entry.operation.clone(), Style::default().fg(Color::Cyan)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Title: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(entry.title.clone()),
            ]),
            Line::from(vec![
                Span::styled("SHA:   ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(entry.full_id.clone(), Style::default().fg(Color::Green)),
            ]),
            Line::from(vec![
                Span::styled("Time:  ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(entry.time.clone()),
            ]),
        ]
    } else {
        vec![]
    }
}

fn build_upstream_details(app: &App) -> Vec<Line<'static>> {
    match &app.upstream_info {
        Some(info) => {
            let mut lines = vec![
                Line::from(vec![
                    Span::styled(
                        "Upstream ",
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{} commits behind", info.behind_count),
                        Style::default().fg(Color::Yellow),
                    ),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        "Latest: ",
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        info.latest_commit.clone(),
                        Style::default().fg(Color::Green),
                    ),
                    Span::raw(" "),
                    Span::raw(info.message.clone()),
                ]),
                Line::from(vec![
                    Span::styled("Date:   ", Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(info.commit_date.clone()),
                ]),
                Line::from(""),
                Line::from(Span::styled(
                    "Upstream commits:",
                    Style::default().add_modifier(Modifier::BOLD),
                )),
            ];

            for uc in &info.commits {
                lines.push(Line::from(vec![
                    Span::styled(uc.id.clone(), Style::default().fg(Color::Green)),
                    Span::raw(" "),
                    Span::raw(uc.message.lines().next().unwrap_or("").to_string()),
                    Span::raw(" "),
                    Span::styled(
                        format!("({}, {})", uc.author, uc.created_at),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            }

            lines
        }
        None => vec![Line::from(Span::styled(
            "Up to date with upstream",
            Style::default().fg(Color::Green),
        ))],
    }
}

// ---------------------------------------------------------------------------
// Command log
// ---------------------------------------------------------------------------

fn render_command_log(f: &mut Frame, app: &App, area: Rect) {
    let visible = app
        .command_log
        .iter()
        .rev()
        .take(area.height.saturating_sub(2) as usize)
        .rev()
        .map(|s| Line::from(Span::styled(s.clone(), Style::default().fg(Color::DarkGray))))
        .collect::<Vec<_>>();

    let block = Paragraph::new(visible).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Log (~) ")
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(block, area);
}

// ---------------------------------------------------------------------------
// Help overlay
// ---------------------------------------------------------------------------

fn render_help_overlay(f: &mut Frame, app: &App, area: Rect) {
    let w = 60u16.min(area.width.saturating_sub(4));
    let h = 24u16.min(area.height.saturating_sub(4));
    let modal = Rect {
        x: (area.width.saturating_sub(w)) / 2,
        y: (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };

    f.render_widget(Clear, modal);

    let lines = vec![
        Line::from(Span::styled(
            "GitButler TUI - Keyboard Shortcuts",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        help_line("q / Ctrl+C", "Quit"),
        help_line("?", "Toggle this help"),
        help_line("Tab / Shift+Tab", "Switch panels"),
        help_line("j / k / ↑ / ↓", "Navigate items"),
        help_line("h / l / ← / →", "Focus details / back"),
        help_line("f", "Fetch from upstream"),
        help_line("Ctrl+R", "Refresh data"),
        help_line("~", "Toggle command log"),
        Line::from(""),
        help_line("c", "Commit changes"),
        help_line("r", "Reword commit (TODO)"),
        help_line("s", "Squash commits (TODO)"),
        help_line("u", "Uncommit (TODO)"),
        help_line("a", "Absorb changes (TODO)"),
        help_line("d", "View diff (TODO)"),
        help_line("R", "Rename branch (TODO)"),
        Line::from(""),
        Line::from(Span::styled(
            "Press ? or Esc to close",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let block = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Help ")
                .border_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
        )
        .scroll((app.help_scroll, 0))
        .wrap(Wrap { trim: false })
        .style(Style::default().bg(Color::Black));

    f.render_widget(block, modal);
}

fn help_line<'a>(key: &'a str, desc: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(
            format!("  {key:<20}"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(desc),
    ])
}

// ---------------------------------------------------------------------------
// Commit modal
// ---------------------------------------------------------------------------

fn render_commit_modal(f: &mut Frame, app: &App, area: Rect) {
    let w = 80u16.min(area.width.saturating_sub(4));
    let h = 30u16.min(area.height.saturating_sub(4));
    let modal = Rect {
        x: (area.width.saturating_sub(w)) / 2,
        y: (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };

    f.render_widget(Clear, modal);

    // Top-level: main content + footer
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(6), Constraint::Length(1)])
        .split(modal);

    // Horizontal split: left (branch + files) | right (subject + message)
    let h_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(outer[0]);

    // Left side: branch selector (4 rows) + file list (rest)
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(4)])
        .split(h_chunks[0]);

    // Right side: subject (3 rows) + message (rest)
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(4)])
        .split(h_chunks[1]);

    // --- Branch selector (top-left) ---
    let branch_focus = app.commit_focus == CommitModalFocus::BranchSelect;
    let branch_border = if branch_focus {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let staged_label = if app.commit_staged_only {
        " [s]taged only "
    } else {
        " [s]taged + unassigned "
    };

    let branch_items: Vec<ListItem> = app
        .commit_branch_options
        .iter()
        .enumerate()
        .map(|(i, opt)| {
            let marker = if i == app.commit_selected_branch {
                ">"
            } else {
                " "
            };
            let label = if opt.is_new_branch {
                format!("{marker} + {}", opt.branch_name)
            } else {
                format!("{marker}   {}", opt.branch_name)
            };
            let style = if i == app.commit_selected_branch {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(Span::styled(label, style)))
        })
        .collect();

    let branch_list = List::new(branch_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Branch{staged_label}"))
            .border_style(branch_border),
    );
    f.render_widget(branch_list, left_chunks[0]);

    // --- File list (bottom-left) ---
    let files_focus = app.commit_focus == CommitModalFocus::Files;
    let files_border = if files_focus {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let file_items: Vec<ListItem> = app
        .commit_files
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let check = if f.selected { "[x]" } else { "[ ]" };
            let cursor = if i == app.commit_file_cursor && files_focus {
                ">"
            } else {
                " "
            };
            let style = if f.selected {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            ListItem::new(Line::from(Span::styled(
                format!("{cursor} {check} {}", f.path),
                style,
            )))
        })
        .collect();

    let file_count = app.commit_files.iter().filter(|f| f.selected).count();
    let file_list = List::new(file_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Files ({file_count} selected, Space to toggle) "))
            .border_style(files_border),
    );
    f.render_widget(file_list, left_chunks[1]);

    // --- Subject (top-right) ---
    let subject_focus = app.commit_focus == CommitModalFocus::Subject;
    let subject_border = if subject_focus {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let subject_display = if app.commit_subject.is_empty() && !subject_focus {
        Span::styled("Enter commit subject...", Style::default().fg(Color::DarkGray))
    } else {
        let cursor = if subject_focus { "_" } else { "" };
        Span::raw(format!("{}{cursor}", app.commit_subject))
    };

    let subject = Paragraph::new(Line::from(subject_display)).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Subject ")
            .border_style(subject_border),
    );
    f.render_widget(subject, right_chunks[0]);

    // --- Message body (bottom-right) ---
    let msg_focus = app.commit_focus == CommitModalFocus::Message;
    let msg_border = if msg_focus {
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let msg_display = if app.commit_message.is_empty() && !msg_focus {
        Span::styled(
            "Optional extended description...",
            Style::default().fg(Color::DarkGray),
        )
    } else {
        let cursor = if msg_focus { "_" } else { "" };
        Span::raw(format!("{}{cursor}", app.commit_message))
    };

    let message = Paragraph::new(Line::from(msg_display))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Message ")
                .border_style(msg_border),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(message, right_chunks[1]);

    // --- Footer with commit button ---
    let button_focused = app.commit_focus == CommitModalFocus::CommitButton;
    let button_style = if button_focused {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    };
    let footer_key = Style::default()
        .fg(Color::Black)
        .bg(Color::DarkGray)
        .add_modifier(Modifier::BOLD);
    let footer_desc = Style::default().fg(Color::DarkGray);
    let footer = Line::from(vec![
        Span::styled(" Commit ", button_style),
        Span::raw("  "),
        Span::styled(" Tab ", footer_key),
        Span::styled("Next field ", footer_desc),
        Span::styled(" s ", footer_key),
        Span::styled("Toggle staged ", footer_desc),
        Span::styled(" Esc ", footer_key),
        Span::styled("Cancel ", footer_desc),
    ]);
    f.render_widget(Paragraph::new(footer), outer[1]);
}

// ---------------------------------------------------------------------------
// Help bar
// ---------------------------------------------------------------------------

fn render_help_bar(f: &mut Frame, app: &App, area: Rect) {
    let key_style = Style::default()
        .fg(Color::Black)
        .bg(Color::DarkGray)
        .add_modifier(Modifier::BOLD);
    let desc_style = Style::default().fg(Color::DarkGray);

    let mut spans: Vec<Span> = Vec::new();

    let mut hotkey = |key: &'static str, desc: &'static str| {
        spans.push(Span::styled(format!(" {key} "), key_style));
        spans.push(Span::styled(format!("{desc} "), desc_style));
    };

    hotkey("?", "Help");
    hotkey("q", "Quit");
    hotkey("Tab", "Panel");
    hotkey("j/k", "Nav");
    hotkey("h/l", "Details");

    match app.active_panel {
        Panel::Upstream | Panel::Status => hotkey("f", "Fetch"),
        Panel::Oplog => {}
    }

    if app.has_uncommitted_changes() {
        hotkey("c", "Commit");
    }

    hotkey("^R", "Refresh");
    hotkey("~", "Log");

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

fn panel_border_style(active: bool) -> Style {
    if active {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

fn commit_status_dot(state: &but_workspace::ui::CommitState) -> Span<'static> {
    match state {
        but_workspace::ui::CommitState::LocalOnly => {
            Span::styled("●", Style::default().fg(Color::White))
        }
        but_workspace::ui::CommitState::LocalAndRemote(_remote_id) => {
            // Check if local and remote match (pushed) vs modified
            // For now, assume green ● for pushed, but we'd need the commit ID to compare
            // Using green for pushed state
            Span::styled("●", Style::default().fg(Color::Green))
        }
        but_workspace::ui::CommitState::Integrated => {
            Span::styled("●", Style::default().fg(Color::Magenta))
        }
    }
}

fn format_relative_time(ms: u128) -> String {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);

    if ms > now_ms {
        return "just now".to_string();
    }

    let diff_secs = ((now_ms - ms) / 1000) as u64;
    if diff_secs < 60 {
        return "just now".to_string();
    }
    let minutes = diff_secs / 60;
    if minutes < 60 {
        return format!("{minutes}m ago");
    }
    let hours = minutes / 60;
    if hours < 24 {
        return format!("{hours}h ago");
    }
    let days = hours / 24;
    format!("{days}d ago")
}

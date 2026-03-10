//! Rendering helpers for the `but link` TUI.

use std::collections::{BTreeMap, BTreeSet};

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::read::ClaimListEntry;

use super::app::App;

/// Minimum width required to show the right-hand agents panel.
const AGENTS_PANEL_MIN_WIDTH: u16 = 72;

/// Fixed width of the right-hand agents panel.
const AGENTS_PANEL_WIDTH: u16 = 42;

/// Render the current TUI frame.
pub(crate) fn render(frame: &mut ratatui::Frame<'_>, app: &App) {
    let root = frame.area();
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(2)])
        .split(root);

    let show_agents_panel = root.width >= AGENTS_PANEL_MIN_WIDTH;
    if show_agents_panel {
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(1), Constraint::Length(AGENTS_PANEL_WIDTH)])
            .split(vertical[0]);
        render_messages(frame, app, horizontal[0]);
        render_agents(frame, app, horizontal[1]);
    } else {
        render_messages(frame, app, vertical[0]);
    }
    render_footer(frame, app, vertical[1]);
}

/// Render the left transcript pane.
fn render_messages(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let mut lines: Vec<Line<'_>> = Vec::new();
    if !app.discoveries.is_empty() {
        lines.push(Line::from(Span::styled(
            "Discoveries",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        for discovery in app.discoveries.iter().take(5) {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    format!("{}: {}", discovery.agent_id, discovery.title),
                    Style::default().fg(Color::Cyan),
                ),
            ]));
            for evidence in discovery.evidence.iter().take(2) {
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::raw(evidence.clone()),
                ]));
            }
        }
        lines.push(Line::raw(""));
    }

    if !app.surfaces.is_empty() {
        lines.push(Line::from(Span::styled(
            "Surfaces",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));
        for surface in app.surfaces.iter().take(8) {
            let path_suffix = if surface.paths.is_empty() {
                String::new()
            } else {
                format!(" [{}]", surface.paths.join(", "))
            };
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    format!(
                        "{} {} {}{}",
                        surface.agent_id, surface.kind, surface.scope, path_suffix
                    ),
                    Style::default().fg(if surface.kind == "intent" {
                        Color::Yellow
                    } else {
                        Color::Magenta
                    }),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::raw(surface.surface.join(", ")),
            ]));
        }
        lines.push(Line::raw(""));
    }

    if !app.messages.is_empty() {
        lines.push(Line::from(Span::styled(
            "Messages",
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::BOLD),
        )));
    }
    for (idx, msg) in app.messages.iter().enumerate() {
        if idx > 0 || !lines.is_empty() {
            lines.push(Line::raw(""));
        }
        lines.push(Line::from(vec![
            Span::styled(
                format!("[{time}] ", time = format_hms_utc(msg.created_at_ms)),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                format!("{agent} ", agent = msg.agent_id),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]));
        for line in msg.content.lines() {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::raw(line.to_owned()),
            ]));
        }
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "(no recent coordination state)",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let inner_height = area.height.saturating_sub(2) as usize;
    let total_lines = lines.len();
    let max_scroll_top = total_lines.saturating_sub(inner_height);
    let scroll_offset = if app.auto_scroll {
        0
    } else {
        app.scroll_offset.min(max_scroll_top)
    };
    let scroll_top = max_scroll_top.saturating_sub(scroll_offset);
    let title_suffix = if app.auto_scroll {
        String::new()
    } else {
        format!(" (scroll +{scroll_offset})")
    };

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Coordination{title_suffix}")),
        )
        .scroll((scroll_top.min(u16::MAX as usize) as u16, 0));
    frame.render_widget(paragraph, area);
}

/// Render the right-hand agents and ownership pane.
fn render_agents(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let mut claims_by_agent: BTreeMap<&str, Vec<&ClaimListEntry>> = BTreeMap::new();
    let mut owners_by_path: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for claim in &app.claims {
        claims_by_agent
            .entry(claim.agent_id.as_str())
            .or_default()
            .push(claim);
        owners_by_path
            .entry(claim.path.as_str())
            .or_default()
            .insert(claim.agent_id.as_str());
    }

    let mut lines: Vec<Line<'_>> = Vec::new();
    if !app.blocks.is_empty() {
        let block_count = app.blocks.len();
        lines.push(Line::from(Span::styled(
            format!("open blocks: {block_count}"),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )));
        for block in app.blocks.iter().take(3) {
            let paths = block.paths.join(", ");
            lines.push(Line::from(Span::styled(
                format!(
                    "  #{id} {agent_id} {reason} [{paths}]",
                    id = block.id,
                    agent_id = block.agent_id,
                    reason = block.reason,
                ),
                if block.mode == "hard" {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default().fg(Color::Yellow)
                },
            )));
        }
        lines.push(Line::raw(""));
    }

    for agent in &app.agents {
        if !lines.is_empty() {
            lines.push(Line::raw(""));
        }
        lines.push(Line::from(vec![
            Span::styled(
                agent.agent_id.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(
                    " (seen {last_seen} · progress {last_progress})",
                    last_seen = relative_age(agent.last_seen_at_ms),
                    last_progress = relative_age(agent.last_progress_at_ms),
                ),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
        if let Some(status) = &agent.status {
            lines.push(Line::from(Span::styled(
                format!("  status: {status}"),
                Style::default().fg(Color::Gray),
            )));
        }
        if let Some(plan) = &agent.plan {
            lines.push(Line::from(Span::styled(
                format!("  plan: {plan}"),
                Style::default().fg(Color::DarkGray),
            )));
        }
        if let Some(claims) = claims_by_agent.get(agent.agent_id.as_str()) {
            let claim_count = claims.len();
            lines.push(Line::from(Span::styled(
                format!("  claims: {claim_count}"),
                Style::default().fg(Color::Yellow),
            )));
            for claim in claims.iter().take(3) {
                let shared = owners_by_path
                    .get(claim.path.as_str())
                    .is_some_and(|owners| owners.len() > 1);
                let prefix = if shared { "  ! " } else { "  - " };
                lines.push(Line::from(Span::styled(
                    format!("{prefix}{path}", path = claim.path),
                    if shared {
                        Style::default().fg(Color::Red)
                    } else {
                        Style::default().fg(Color::Gray)
                    },
                )));
            }
        }
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "(no active agents)",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::TOP | Borders::RIGHT | Borders::BOTTOM)
            .title(format!(
                " Agents · {stale_minutes}m ({agent_count})",
                stale_minutes = crate::read::TUI_AGENT_STALE_MS / 60_000,
                agent_count = app.agents.len(),
            )),
    );
    frame.render_widget(paragraph, area);
}

/// Render the footer area with key hints or persistent errors.
fn render_footer(frame: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let border = Block::default().borders(Borders::TOP);
    let inner = border.inner(area);
    frame.render_widget(border, area);

    let paragraph = if let Some(err) = &app.last_error {
        Paragraph::new(Line::from(vec![
            Span::styled("DB error: ", Style::default().fg(Color::Red)),
            Span::styled(err.clone(), Style::default().fg(Color::Red)),
            Span::styled(" (retrying)", Style::default().fg(Color::DarkGray)),
        ]))
    } else {
        Paragraph::new(Line::from(Span::styled(
            "q quit | Up/Down scroll | PgUp/PgDn page | Home/End",
            Style::default().fg(Color::DarkGray),
        )))
    };
    frame.render_widget(paragraph, inner);
}

/// Format a UTC timestamp as `HH:MM:SS`.
fn format_hms_utc(created_at_ms: i64) -> String {
    let seconds = created_at_ms.div_euclid(1000);
    let sec_in_day = seconds.rem_euclid(86_400);
    let hour = sec_in_day / 3_600;
    let minute = (sec_in_day % 3_600) / 60;
    let second = sec_in_day % 60;
    format!("{hour:02}:{minute:02}:{second:02}")
}

/// Format a relative age label for the agents panel.
fn relative_age(updated_at_ms: i64) -> String {
    let now_ms = crate::db::now_unix_ms().unwrap_or(updated_at_ms);
    let delta_ms = now_ms.saturating_sub(updated_at_ms).max(0);
    let delta_s = delta_ms / 1000;
    if delta_s < 60 {
        "now".to_owned()
    } else if delta_s < 3_600 {
        format!("{minutes}m", minutes = delta_s / 60)
    } else if delta_s < 86_400 {
        format!("{hours}h", hours = delta_s / 3_600)
    } else {
        format!("{days}d", days = delta_s / 86_400)
    }
}

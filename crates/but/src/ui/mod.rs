use std::{io, ops::ControlFlow};

pub mod test_utils;
use anyhow::Result;
use bstr::ByteSlice;
use ratatui::{
    crossterm::event::{self},
    style::Stylize,
    widgets::Widget,
};

/// Run a terminal UI to select branches from the given stacks.
pub fn run_branch_selector_ui(stacks: Vec<SimpleStack>) -> Result<Vec<String>> {
    let mut terminal = ratatui::init();
    let mut state = BranchSelectorState {
        stacks,
        ..Default::default()
    };
    state.run(&mut terminal)?;
    ratatui::restore();

    Ok(state.selected_branches)
}

#[derive(Debug, Default)]
pub struct SimpleCommit {
    /// The commit title.
    pub title: String,
    /// The short SHA of the commit.
    pub sha: String,
}

impl From<but_workspace::ui::Commit> for SimpleCommit {
    fn from(commit: but_workspace::ui::Commit) -> Self {
        let short_sha = format!("{:.7}", commit.id);
        Self {
            title: commit
                .message
                .lines()
                .next()
                .and_then(|line| line.to_str().ok())
                .unwrap_or("")
                .to_string(),
            sha: short_sha,
        }
    }
}

#[derive(Debug, Default)]
pub struct SimpleBranch {
    /// The name of the branch.
    pub name: String,
    /// The commits in the branch.
    /// Ordered from child to parent.
    pub commits: Vec<SimpleCommit>,
    /// The reviews associated with the given branch.
    /// Already formatted for display, e.g '#1234' for GitHub PRs.
    pub reviews: Vec<String>,
}

#[derive(Debug, Default)]
pub struct SimpleStack {
    /// The branches in the stack.
    /// Orderd from child to parent.
    pub branches: Vec<SimpleBranch>,
}

#[derive(Debug, Default)]
pub struct BranchSelectorState {
    /// The stacks in the workspace.
    pub stacks: Vec<SimpleStack>,
    /// The selected branches.
    pub selected_branches: Vec<String>,
    /// The branch being focused.
    pub focused_branch: Option<String>,
    exit: bool,
}

impl BranchSelectorState {
    /// runs the application's main loop until the user quits
    pub fn run(&mut self, terminal: &mut ratatui::DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn draw(&self, frame: &mut ratatui::Frame) {
        frame.render_widget(self, frame.area());
    }

    /// updates the application's state based on user input
    fn handle_events(&mut self) -> io::Result<()> {
        match event::read()? {
            // it's important to check that the event is a key press event as
            // crossterm also emits key release and repeat events on Windows.
            event::Event::Key(key_event) if key_event.kind == event::KeyEventKind::Press => {
                self.handle_key_event(key_event)
            }
            _ => {}
        };
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: event::KeyEvent) {
        match key_event.code {
            event::KeyCode::Enter => self.exit(),
            event::KeyCode::Char('q') => self.cancel(),
            event::KeyCode::Char('a') => {
                // Unselect all if all are selected
                let all_branches = self.stacks.iter().fold(0, |acc, s| acc + s.branches.len());
                if self.selected_branches.len() == all_branches {
                    self.selected_branches.clear();
                    return;
                }
                // Select all branches
                self.selected_branches.clear();
                for stack in &self.stacks {
                    for branch in &stack.branches {
                        self.selected_branches.push(branch.name.clone());
                    }
                }
            }
            event::KeyCode::Char('s') => {
                // Toggle the selection of the focused branch
                if let Some(focused) = &self.focused_branch {
                    if !self.selected_branches.contains(focused) {
                        self.selected_branches.push(focused.clone());
                    } else {
                        self.selected_branches.retain(|b| b != focused);
                    }
                }
            }
            event::KeyCode::Down => {
                // Move focus to the next branch
                self.move_focus_down();
            }
            event::KeyCode::Up => {
                // Move focus to the previous branch
                self.move_focus_up();
            }
            event::KeyCode::Left => {
                // Move focus to the previous stack
                self.move_focus_left();
            }
            event::KeyCode::Right => {
                // Move focus to the next stack
                self.move_focus_right();
            }
            _ => {}
        }
    }

    /// Move focus to the previous stack in the workspace.
    fn move_focus_left(&mut self) {
        if let Some(current_focus) = &self.focused_branch
            && self.stacks.len() > 1
        {
            let stacks_len = self.stacks.len();
            let mut stack_heads = self
                .stacks
                .iter()
                .map(|stack| {
                    stack
                        .branches
                        .iter()
                        .map(|b| b.name.clone())
                        .collect::<Vec<_>>()
                })
                .enumerate();
            let mut current_index = None;
            for (idx, heads) in stack_heads.clone() {
                if heads.contains(current_focus) {
                    current_index = Some(idx);
                    break;
                }
            }

            if let Some(current_idx) = current_index {
                let prev_idx = if current_idx == 0 {
                    stacks_len - 1
                } else {
                    current_idx - 1
                };
                let head_name = stack_heads.find_map(|(i, h)| {
                    if i == prev_idx {
                        h.first().cloned()
                    } else {
                        None
                    }
                });

                self.focused_branch = head_name;
            }
        } else if let Some(first_branch) =
            self.stacks.first().and_then(|stack| stack.branches.first())
        {
            self.focused_branch = Some(first_branch.name.clone());
        }
    }

    /// Move focus to the next stack in the workspace.
    fn move_focus_right(&mut self) {
        if let Some(current_focus) = &self.focused_branch
            && self.stacks.len() > 1
        {
            let stacks_len = self.stacks.len();
            let mut stack_heads = self
                .stacks
                .iter()
                .map(|stack| {
                    stack
                        .branches
                        .iter()
                        .map(|b| b.name.clone())
                        .collect::<Vec<_>>()
                })
                .enumerate();
            let mut current_index = None;
            for (idx, heads) in stack_heads.clone() {
                if heads.contains(current_focus) {
                    current_index = Some(idx);
                    break;
                }
            }

            if let Some(current_idx) = current_index {
                let next_idx = if current_idx == stacks_len - 1 {
                    0
                } else {
                    current_idx + 1
                };
                let head_name = stack_heads.find_map(|(i, h)| {
                    if i == next_idx {
                        h.first().cloned()
                    } else {
                        None
                    }
                });

                self.focused_branch = head_name;
            }
        } else if let Some(first_branch) =
            self.stacks.first().and_then(|stack| stack.branches.first())
        {
            self.focused_branch = Some(first_branch.name.clone());
        }
    }

    /// Move focus to the previous branch in the stack.
    fn move_focus_up(&mut self) {
        if let Some(current_focus) = &self.focused_branch {
            let mut previous_branch: Option<String> = None;
            'outer: for stack in &self.stacks {
                for branch in &stack.branches {
                    if &branch.name == current_focus {
                        if let Some(prev) = previous_branch {
                            self.focused_branch = Some(prev);
                        }
                        break 'outer;
                    }
                    previous_branch = Some(branch.name.clone());
                }
            }
        } else if let Some(first_branch) =
            self.stacks.first().and_then(|stack| stack.branches.first())
        {
            self.focused_branch = Some(first_branch.name.clone());
        }
    }

    /// Move focus to the next branch in the stack.
    fn move_focus_down(&mut self) {
        if let Some(current_focus) = &self.focused_branch {
            let mut found = false;
            'outer: for stack in &self.stacks {
                for branch in &stack.branches {
                    if found {
                        self.focused_branch = Some(branch.name.clone());
                        break 'outer;
                    }
                    if &branch.name == current_focus {
                        found = true;
                    }
                }
            }
        } else if let Some(first_branch) =
            self.stacks.first().and_then(|stack| stack.branches.first())
        {
            self.focused_branch = Some(first_branch.name.clone());
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }

    fn cancel(&mut self) {
        self.selected_branches.clear();
        self.exit = true;
    }
}

impl ratatui::widgets::Widget for &BranchSelectorState {
    fn render(self, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
        let title = ratatui::text::Line::from("Workspace".bold());
        let instructions = ratatui::text::Line::from(vec![
            " Done ".into(),
            "<Enter> ".blue().bold(),
            "Cancel ".into(),
            "<Q> ".red().bold(),
            "Select branch ".into(),
            "<S> ".green(),
            "Select all ".into(),
            "<A> ".green(),
            "Move with arrow keys ".into(),
        ]);
        let workspace_block = ratatui::widgets::Block::bordered()
            .title(title.centered())
            .title_bottom(instructions.centered())
            .border_set(ratatui::symbols::border::THICK);

        if self.stacks.is_empty() {
            render_empty_workspace(area, buf, workspace_block);
            return;
        }

        let stack_constraints: Vec<ratatui::layout::Constraint> = self
            .stacks
            .iter()
            .map(|_| ratatui::layout::Constraint::Percentage(100 / self.stacks.len() as u16))
            .collect();

        let inner_area = workspace_block.inner(area);
        let stack_layout = ratatui::layout::Layout::horizontal(stack_constraints).split(inner_area);
        workspace_block.render(area, buf);

        for (stack_idx, stack) in self.stacks.iter().enumerate() {
            if let ControlFlow::Break(_) = render_stack(self, buf, &stack_layout, stack_idx, stack)
            {
                break;
            }
        }
    }
}

/// Render a single stack of branches.
fn render_stack(
    state: &BranchSelectorState,
    buf: &mut ratatui::prelude::Buffer,
    stack_layout: &std::rc::Rc<[ratatui::prelude::Rect]>,
    stack_idx: usize,
    stack: &SimpleStack,
) -> ControlFlow<()> {
    if stack_idx >= stack_layout.len() {
        return ControlFlow::Break(());
    }
    let branch_constraints: Vec<ratatui::layout::Constraint> = stack
        .branches
        .iter()
        .map(|_| ratatui::layout::Constraint::Length(3))
        .collect();
    let branch_layout =
        ratatui::layout::Layout::vertical(branch_constraints).split(stack_layout[stack_idx]);

    for (branch_idx, branch) in stack.branches.iter().enumerate() {
        if let ControlFlow::Break(_) = render_branch(state, buf, &branch_layout, branch_idx, branch)
        {
            break;
        }
    }
    ControlFlow::Continue(())
}

/// Render a single branch in the given area.
fn render_branch(
    state: &BranchSelectorState,
    buf: &mut ratatui::prelude::Buffer,
    branch_layout: &std::rc::Rc<[ratatui::prelude::Rect]>,
    branch_idx: usize,
    branch: &SimpleBranch,
) -> ControlFlow<()> {
    if branch_idx >= branch_layout.len() {
        return ControlFlow::Break(());
    }

    let is_focused = state.focused_branch.as_ref() == Some(&branch.name);
    let is_selected = state.selected_branches.contains(&branch.name);

    let selected_prefix = if is_selected { " ★ " } else { "" };

    let mut branch_block =
        ratatui::widgets::Block::bordered().title(format!("{}{}", selected_prefix, branch.name));

    if is_focused {
        branch_block = branch_block
            .border_set(ratatui::symbols::border::DOUBLE)
            .border_style(ratatui::style::Style::default().fg(ratatui::style::Color::LightBlue));
    }

    let commit_count = format!("{} commits", branch.commits.len());
    let inner = branch_block.inner(branch_layout[branch_idx]);
    let mut branch_content: Vec<ratatui::text::Span<'_>> = vec![commit_count.into()];

    if !branch.reviews.is_empty() {
        branch_content.push(" * ".into());
        branch_content.push("(".blue());
        branch_content.push(branch.reviews.join(", ").blue());
        branch_content.push(")".blue());
    }

    let mut line = ratatui::text::Line::from(branch_content);
    if is_selected {
        line = line.style(
            ratatui::style::Style::default()
                .fg(ratatui::style::Color::Black)
                .bg(ratatui::style::Color::White),
        );
    }

    line.render(inner, buf);
    branch_block.render(branch_layout[branch_idx], buf);
    ControlFlow::Continue(())
}

/// Render a message indicating that there are no branches in the workspace.
fn render_empty_workspace(
    area: ratatui::prelude::Rect,
    buf: &mut ratatui::prelude::Buffer,
    block: ratatui::widgets::Block<'_>,
) {
    let empty_state = ratatui::text::Text::from(vec![ratatui::text::Line::from(vec![
        "No branches in workspace".into(),
    ])]);

    let inner = block.inner(area);
    block.render(area, buf);

    let vertical_center = inner.height / 2;
    let centered_area = ratatui::layout::Rect {
        y: inner.y + vertical_center,
        height: 1,
        ..inner
    };

    ratatui::widgets::Paragraph::new(empty_state)
        .centered()
        .render(centered_area, buf);
}

#[cfg(test)]
mod tests {
    use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};

    use super::*;
    use crate::ui::test_utils::buffer_to_string;

    #[test]
    fn render_empty_workspace() {
        let selector = BranchSelectorState::default();
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 9));

        selector.render(buf.area, &mut buf);

        let buffer_str = buffer_to_string(&buf);
        insta::assert_snapshot!(buffer_str, @r"
        ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━Workspace━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
        ┃                                                                              ┃
        ┃                                                                              ┃
        ┃                                                                              ┃
        ┃                           No branches in workspace                           ┃
        ┃                                                                              ┃
        ┃                                                                              ┃
        ┃                                                                              ┃
        ┗ Done <Enter> Cancel <Q> Select branch <S> Select all <A> Move with arrow keys┛
        ");
    }

    #[test]
    fn render_workspace_with_one_branch() {
        let selector = BranchSelectorState {
            stacks: vec![SimpleStack {
                branches: vec![SimpleBranch {
                    name: "branch-1".into(),
                    reviews: vec![String::from("#1234")],
                    ..Default::default()
                }],
            }],
            ..Default::default()
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 10));

        selector.render(buf.area, &mut buf);
        let buffer_str = buffer_to_string(&buf);
        insta::assert_snapshot!(buffer_str, @r"
        ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━Workspace━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
        ┃┌branch-1────────────────────────────────────────────────────────────────────┐┃
        ┃│0 commits * (#1234)                                                         │┃
        ┃└────────────────────────────────────────────────────────────────────────────┘┃
        ┃                                                                              ┃
        ┃                                                                              ┃
        ┃                                                                              ┃
        ┃                                                                              ┃
        ┃                                                                              ┃
        ┗ Done <Enter> Cancel <Q> Select branch <S> Select all <A> Move with arrow keys┛
        ");
    }

    #[test]
    fn render_workspace_with_two_branch() {
        let selector = BranchSelectorState {
            stacks: vec![
                SimpleStack {
                    branches: vec![SimpleBranch {
                        name: "branch-1".into(),
                        ..Default::default()
                    }],
                },
                SimpleStack {
                    branches: vec![SimpleBranch {
                        name: "branch-2".into(),
                        ..Default::default()
                    }],
                },
            ],
            ..Default::default()
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 10));

        selector.render(buf.area, &mut buf);
        let buffer_str = buffer_to_string(&buf);
        insta::assert_snapshot!(buffer_str, @r"
        ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━Workspace━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
        ┃┌branch-1─────────────────────────────┐┌branch-2─────────────────────────────┐┃
        ┃│0 commits                            ││0 commits                            │┃
        ┃└─────────────────────────────────────┘└─────────────────────────────────────┘┃
        ┃                                                                              ┃
        ┃                                                                              ┃
        ┃                                                                              ┃
        ┃                                                                              ┃
        ┃                                                                              ┃
        ┗ Done <Enter> Cancel <Q> Select branch <S> Select all <A> Move with arrow keys┛
        ");
    }

    #[test]
    fn render_workspace_with_two_stacked_branches() {
        let branch_1_commits = vec![
            SimpleCommit {
                title: "Add README".into(),
                sha: "d4e5f62".into(),
            },
            SimpleCommit {
                title: "Initial commit".into(),
                sha: "a1b2c3d".into(),
            },
        ];

        let branch_2_commits = vec![
            SimpleCommit {
                title: "Update the LICENSE".into(),
                sha: "3434344".into(),
            },
            SimpleCommit {
                title: "Add LICENSE".into(),
                sha: "2222222".into(),
            },
        ];

        let selector = BranchSelectorState {
            stacks: vec![SimpleStack {
                branches: vec![
                    SimpleBranch {
                        name: "branch-1".into(),
                        commits: branch_1_commits,
                        ..Default::default()
                    },
                    SimpleBranch {
                        name: "branch-2".into(),
                        commits: branch_2_commits,
                        ..Default::default()
                    },
                ],
            }],
            ..Default::default()
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 10));

        selector.render(buf.area, &mut buf);
        let buffer_str = buffer_to_string(&buf);
        insta::assert_snapshot!(buffer_str, @r"
        ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━Workspace━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
        ┃┌branch-1────────────────────────────────────────────────────────────────────┐┃
        ┃│2 commits                                                                   │┃
        ┃└────────────────────────────────────────────────────────────────────────────┘┃
        ┃┌branch-2────────────────────────────────────────────────────────────────────┐┃
        ┃│2 commits                                                                   │┃
        ┃└────────────────────────────────────────────────────────────────────────────┘┃
        ┃                                                                              ┃
        ┃                                                                              ┃
        ┗ Done <Enter> Cancel <Q> Select branch <S> Select all <A> Move with arrow keys┛
        ");
    }

    #[test]
    fn render_workspace_with_focused_branch() {
        let selector = BranchSelectorState {
            stacks: vec![SimpleStack {
                branches: vec![
                    SimpleBranch {
                        name: "branch-1".into(),
                        commits: vec![SimpleCommit {
                            title: "First commit".into(),
                            sha: "abc1234".into(),
                        }],
                        ..Default::default()
                    },
                    SimpleBranch {
                        name: "branch-2".into(),
                        commits: vec![SimpleCommit {
                            title: "Second commit".into(),
                            sha: "def5678".into(),
                        }],
                        ..Default::default()
                    },
                ],
            }],
            focused_branch: Some("branch-2".into()),
            ..Default::default()
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 10));

        selector.render(buf.area, &mut buf);
        let buffer_str = buffer_to_string(&buf);
        insta::assert_snapshot!(buffer_str, @r"
        ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━Workspace━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
        ┃┌branch-1────────────────────────────────────────────────────────────────────┐┃
        ┃│1 commits                                                                   │┃
        ┃└────────────────────────────────────────────────────────────────────────────┘┃
        ┃╔branch-2════════════════════════════════════════════════════════════════════╗┃
        ┃║1 commits                                                                   ║┃
        ┃╚════════════════════════════════════════════════════════════════════════════╝┃
        ┃                                                                              ┃
        ┃                                                                              ┃
        ┗ Done <Enter> Cancel <Q> Select branch <S> Select all <A> Move with arrow keys┛
        ");
    }

    #[test]
    fn render_workspace_with_selected_branch() {
        let selector = BranchSelectorState {
            stacks: vec![SimpleStack {
                branches: vec![
                    SimpleBranch {
                        name: "branch-1".into(),
                        commits: vec![SimpleCommit {
                            title: "First commit".into(),
                            sha: "abc1234".into(),
                        }],
                        ..Default::default()
                    },
                    SimpleBranch {
                        name: "branch-2".into(),
                        commits: vec![SimpleCommit {
                            title: "Second commit".into(),
                            sha: "def5678".into(),
                        }],
                        ..Default::default()
                    },
                ],
            }],
            selected_branches: vec!["branch-1".into()],
            ..Default::default()
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 10));

        selector.render(buf.area, &mut buf);
        let buffer_str = buffer_to_string(&buf);
        insta::assert_snapshot!(buffer_str, @r"
        ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━Workspace━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
        ┃┌ ★ branch-1─────────────────────────────────────────────────────────────────┐┃
        ┃│1 commits                                                                   │┃
        ┃└────────────────────────────────────────────────────────────────────────────┘┃
        ┃┌branch-2────────────────────────────────────────────────────────────────────┐┃
        ┃│1 commits                                                                   │┃
        ┃└────────────────────────────────────────────────────────────────────────────┘┃
        ┃                                                                              ┃
        ┃                                                                              ┃
        ┗ Done <Enter> Cancel <Q> Select branch <S> Select all <A> Move with arrow keys┛
        ");
    }

    #[test]
    fn render_workspace_with_focused_and_selected_branch() {
        let selector = BranchSelectorState {
            stacks: vec![SimpleStack {
                branches: vec![
                    SimpleBranch {
                        name: "branch-1".into(),
                        commits: vec![SimpleCommit {
                            title: "First commit".into(),
                            sha: "abc1234".into(),
                        }],
                        ..Default::default()
                    },
                    SimpleBranch {
                        name: "branch-2".into(),
                        commits: vec![SimpleCommit {
                            title: "Second commit".into(),
                            sha: "def5678".into(),
                        }],
                        ..Default::default()
                    },
                ],
            }],
            focused_branch: Some("branch-1".into()),
            selected_branches: vec!["branch-1".into()],
            ..Default::default()
        };
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 10));

        selector.render(buf.area, &mut buf);
        let buffer_str = buffer_to_string(&buf);
        insta::assert_snapshot!(buffer_str, @r"
        ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━Workspace━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
        ┃╔ ★ branch-1═════════════════════════════════════════════════════════════════╗┃
        ┃║1 commits                                                                   ║┃
        ┃╚════════════════════════════════════════════════════════════════════════════╝┃
        ┃┌branch-2────────────────────────────────────────────────────────────────────┐┃
        ┃│1 commits                                                                   │┃
        ┃└────────────────────────────────────────────────────────────────────────────┘┃
        ┃                                                                              ┃
        ┃                                                                              ┃
        ┗ Done <Enter> Cancel <Q> Select branch <S> Select all <A> Move with arrow keys┛
        ");
    }

    #[test]
    fn move_focus_up_and_down() {
        let mut selector = BranchSelectorState {
            stacks: vec![SimpleStack {
                branches: vec![
                    SimpleBranch {
                        name: "branch-1".into(),
                        commits: vec![],
                        ..Default::default()
                    },
                    SimpleBranch {
                        name: "branch-2".into(),
                        commits: vec![],
                        ..Default::default()
                    },
                    SimpleBranch {
                        name: "branch-3".into(),
                        commits: vec![],
                        ..Default::default()
                    },
                ],
            }],
            ..Default::default()
        };

        // Initial state: no focus, no selection
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 12));
        selector.render(buf.area, &mut buf);
        let buffer_str = buffer_to_string(&buf);
        insta::assert_snapshot!(buffer_str, @r"
        ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━Workspace━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
        ┃┌branch-1────────────────────────────────────────────────────────────────────┐┃
        ┃│0 commits                                                                   │┃
        ┃└────────────────────────────────────────────────────────────────────────────┘┃
        ┃┌branch-2────────────────────────────────────────────────────────────────────┐┃
        ┃│0 commits                                                                   │┃
        ┃└────────────────────────────────────────────────────────────────────────────┘┃
        ┃┌branch-3────────────────────────────────────────────────────────────────────┐┃
        ┃│0 commits                                                                   │┃
        ┃└────────────────────────────────────────────────────────────────────────────┘┃
        ┃                                                                              ┃
        ┗ Done <Enter> Cancel <Q> Select branch <S> Select all <A> Move with arrow keys┛
        ");

        // Move focus down to branch-1
        selector.handle_key_event(event::KeyCode::Down.into());
        assert_eq!(selector.focused_branch, Some("branch-1".into()));
        buf = Buffer::empty(Rect::new(0, 0, 80, 12));
        selector.render(buf.area, &mut buf);
        let buffer_str = buffer_to_string(&buf);
        insta::assert_snapshot!(buffer_str, @r"
        ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━Workspace━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
        ┃╔branch-1════════════════════════════════════════════════════════════════════╗┃
        ┃║0 commits                                                                   ║┃
        ┃╚════════════════════════════════════════════════════════════════════════════╝┃
        ┃┌branch-2────────────────────────────────────────────────────────────────────┐┃
        ┃│0 commits                                                                   │┃
        ┃└────────────────────────────────────────────────────────────────────────────┘┃
        ┃┌branch-3────────────────────────────────────────────────────────────────────┐┃
        ┃│0 commits                                                                   │┃
        ┃└────────────────────────────────────────────────────────────────────────────┘┃
        ┃                                                                              ┃
        ┗ Done <Enter> Cancel <Q> Select branch <S> Select all <A> Move with arrow keys┛
        ");

        // Move focus down to branch-2 (middle)
        selector.handle_key_event(event::KeyCode::Down.into());
        assert_eq!(selector.focused_branch, Some("branch-2".into()));
        buf = Buffer::empty(Rect::new(0, 0, 80, 12));
        selector.render(buf.area, &mut buf);
        let buffer_str = buffer_to_string(&buf);
        insta::assert_snapshot!(buffer_str, @r"
        ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━Workspace━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
        ┃┌branch-1────────────────────────────────────────────────────────────────────┐┃
        ┃│0 commits                                                                   │┃
        ┃└────────────────────────────────────────────────────────────────────────────┘┃
        ┃╔branch-2════════════════════════════════════════════════════════════════════╗┃
        ┃║0 commits                                                                   ║┃
        ┃╚════════════════════════════════════════════════════════════════════════════╝┃
        ┃┌branch-3────────────────────────────────────────────────────────────────────┐┃
        ┃│0 commits                                                                   │┃
        ┃└────────────────────────────────────────────────────────────────────────────┘┃
        ┃                                                                              ┃
        ┗ Done <Enter> Cancel <Q> Select branch <S> Select all <A> Move with arrow keys┛
        ");

        // Select branch-2
        selector.handle_key_event(event::KeyCode::Char('s').into());
        assert_eq!(selector.selected_branches, vec!["branch-2".to_string()]);
        buf = Buffer::empty(Rect::new(0, 0, 80, 12));
        selector.render(buf.area, &mut buf);
        let buffer_str = buffer_to_string(&buf);
        insta::assert_snapshot!(buffer_str, @r"
        ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━Workspace━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
        ┃┌branch-1────────────────────────────────────────────────────────────────────┐┃
        ┃│0 commits                                                                   │┃
        ┃└────────────────────────────────────────────────────────────────────────────┘┃
        ┃╔ ★ branch-2═════════════════════════════════════════════════════════════════╗┃
        ┃║0 commits                                                                   ║┃
        ┃╚════════════════════════════════════════════════════════════════════════════╝┃
        ┃┌branch-3────────────────────────────────────────────────────────────────────┐┃
        ┃│0 commits                                                                   │┃
        ┃└────────────────────────────────────────────────────────────────────────────┘┃
        ┃                                                                              ┃
        ┗ Done <Enter> Cancel <Q> Select branch <S> Select all <A> Move with arrow keys┛
        ");
    }

    #[test]
    fn move_focus_right_between_stacks() {
        let mut selector = BranchSelectorState {
            stacks: vec![
                SimpleStack {
                    branches: vec![
                        SimpleBranch {
                            name: "stack-1-branch-1".into(),
                            commits: vec![],
                            ..Default::default()
                        },
                        SimpleBranch {
                            name: "stack-1-branch-2".into(),
                            commits: vec![],
                            ..Default::default()
                        },
                    ],
                },
                SimpleStack {
                    branches: vec![
                        SimpleBranch {
                            name: "stack-2-branch-1".into(),
                            commits: vec![],
                            ..Default::default()
                        },
                        SimpleBranch {
                            name: "stack-2-branch-2".into(),
                            commits: vec![],
                            ..Default::default()
                        },
                    ],
                },
            ],
            ..Default::default()
        };

        // Initial state: no focus
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 10));
        selector.render(buf.area, &mut buf);
        let buffer_str = buffer_to_string(&buf);
        insta::assert_snapshot!(buffer_str, @r"
        ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━Workspace━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
        ┃┌stack-1-branch-1─────────────────────┐┌stack-2-branch-1─────────────────────┐┃
        ┃│0 commits                            ││0 commits                            │┃
        ┃└─────────────────────────────────────┘└─────────────────────────────────────┘┃
        ┃┌stack-1-branch-2─────────────────────┐┌stack-2-branch-2─────────────────────┐┃
        ┃│0 commits                            ││0 commits                            │┃
        ┃└─────────────────────────────────────┘└─────────────────────────────────────┘┃
        ┃                                                                              ┃
        ┃                                                                              ┃
        ┗ Done <Enter> Cancel <Q> Select branch <S> Select all <A> Move with arrow keys┛
        ");

        // Move focus right to first stack's head (stack-1-branch-1)
        selector.handle_key_event(event::KeyCode::Right.into());
        assert_eq!(selector.focused_branch, Some("stack-1-branch-1".into()));
        buf = Buffer::empty(Rect::new(0, 0, 80, 10));
        selector.render(buf.area, &mut buf);
        let buffer_str = buffer_to_string(&buf);
        insta::assert_snapshot!(buffer_str, @r"
        ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━Workspace━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
        ┃╔stack-1-branch-1═════════════════════╗┌stack-2-branch-1─────────────────────┐┃
        ┃║0 commits                            ║│0 commits                            │┃
        ┃╚═════════════════════════════════════╝└─────────────────────────────────────┘┃
        ┃┌stack-1-branch-2─────────────────────┐┌stack-2-branch-2─────────────────────┐┃
        ┃│0 commits                            ││0 commits                            │┃
        ┃└─────────────────────────────────────┘└─────────────────────────────────────┘┃
        ┃                                                                              ┃
        ┃                                                                              ┃
        ┗ Done <Enter> Cancel <Q> Select branch <S> Select all <A> Move with arrow keys┛
        ");

        // Move focus right to second stack's head (stack-2-branch-1)
        selector.handle_key_event(event::KeyCode::Right.into());
        assert_eq!(selector.focused_branch, Some("stack-2-branch-1".into()));
        buf = Buffer::empty(Rect::new(0, 0, 80, 10));
        selector.render(buf.area, &mut buf);
        let buffer_str = buffer_to_string(&buf);
        insta::assert_snapshot!(buffer_str, @r"
        ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━Workspace━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
        ┃┌stack-1-branch-1─────────────────────┐╔stack-2-branch-1═════════════════════╗┃
        ┃│0 commits                            │║0 commits                            ║┃
        ┃└─────────────────────────────────────┘╚═════════════════════════════════════╝┃
        ┃┌stack-1-branch-2─────────────────────┐┌stack-2-branch-2─────────────────────┐┃
        ┃│0 commits                            ││0 commits                            │┃
        ┃└─────────────────────────────────────┘└─────────────────────────────────────┘┃
        ┃                                                                              ┃
        ┃                                                                              ┃
        ┗ Done <Enter> Cancel <Q> Select branch <S> Select all <A> Move with arrow keys┛
        ");

        // Move focus right again, should wrap to first stack's head
        selector.handle_key_event(event::KeyCode::Right.into());
        assert_eq!(selector.focused_branch, Some("stack-1-branch-1".into()));
        buf = Buffer::empty(Rect::new(0, 0, 80, 10));
        selector.render(buf.area, &mut buf);
        let buffer_str = buffer_to_string(&buf);
        insta::assert_snapshot!(buffer_str, @r"
        ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━Workspace━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
        ┃╔stack-1-branch-1═════════════════════╗┌stack-2-branch-1─────────────────────┐┃
        ┃║0 commits                            ║│0 commits                            │┃
        ┃╚═════════════════════════════════════╝└─────────────────────────────────────┘┃
        ┃┌stack-1-branch-2─────────────────────┐┌stack-2-branch-2─────────────────────┐┃
        ┃│0 commits                            ││0 commits                            │┃
        ┃└─────────────────────────────────────┘└─────────────────────────────────────┘┃
        ┃                                                                              ┃
        ┃                                                                              ┃
        ┗ Done <Enter> Cancel <Q> Select branch <S> Select all <A> Move with arrow keys┛
        ");
    }

    #[test]
    fn move_focus_left_between_stacks() {
        let mut selector = BranchSelectorState {
            stacks: vec![
                SimpleStack {
                    branches: vec![
                        SimpleBranch {
                            name: "stack-1-branch-1".into(),
                            commits: vec![],
                            ..Default::default()
                        },
                        SimpleBranch {
                            name: "stack-1-branch-2".into(),
                            commits: vec![],
                            ..Default::default()
                        },
                    ],
                },
                SimpleStack {
                    branches: vec![
                        SimpleBranch {
                            name: "stack-2-branch-1".into(),
                            commits: vec![],
                            ..Default::default()
                        },
                        SimpleBranch {
                            name: "stack-2-branch-2".into(),
                            commits: vec![],
                            ..Default::default()
                        },
                    ],
                },
            ],
            ..Default::default()
        };

        // Initial state: no focus
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 10));
        selector.render(buf.area, &mut buf);
        let buffer_str = buffer_to_string(&buf);
        insta::assert_snapshot!(buffer_str, @r"
        ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━Workspace━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
        ┃┌stack-1-branch-1─────────────────────┐┌stack-2-branch-1─────────────────────┐┃
        ┃│0 commits                            ││0 commits                            │┃
        ┃└─────────────────────────────────────┘└─────────────────────────────────────┘┃
        ┃┌stack-1-branch-2─────────────────────┐┌stack-2-branch-2─────────────────────┐┃
        ┃│0 commits                            ││0 commits                            │┃
        ┃└─────────────────────────────────────┘└─────────────────────────────────────┘┃
        ┃                                                                              ┃
        ┃                                                                              ┃
        ┗ Done <Enter> Cancel <Q> Select branch <S> Select all <A> Move with arrow keys┛
        ");

        // Move focus left to first stack's head (stack-1-branch-1)
        selector.handle_key_event(event::KeyCode::Left.into());
        assert_eq!(selector.focused_branch, Some("stack-1-branch-1".into()));
        buf = Buffer::empty(Rect::new(0, 0, 80, 10));
        selector.render(buf.area, &mut buf);
        let buffer_str = buffer_to_string(&buf);
        insta::assert_snapshot!(buffer_str, @r"
        ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━Workspace━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
        ┃╔stack-1-branch-1═════════════════════╗┌stack-2-branch-1─────────────────────┐┃
        ┃║0 commits                            ║│0 commits                            │┃
        ┃╚═════════════════════════════════════╝└─────────────────────────────────────┘┃
        ┃┌stack-1-branch-2─────────────────────┐┌stack-2-branch-2─────────────────────┐┃
        ┃│0 commits                            ││0 commits                            │┃
        ┃└─────────────────────────────────────┘└─────────────────────────────────────┘┃
        ┃                                                                              ┃
        ┃                                                                              ┃
        ┗ Done <Enter> Cancel <Q> Select branch <S> Select all <A> Move with arrow keys┛
        ");

        // Move focus down again to first stack's second branch
        selector.handle_key_event(event::KeyCode::Down.into());
        assert_eq!(selector.focused_branch, Some("stack-1-branch-2".into()));
        buf = Buffer::empty(Rect::new(0, 0, 80, 10));
        selector.render(buf.area, &mut buf);
        let buffer_str = buffer_to_string(&buf);
        insta::assert_snapshot!(buffer_str, @r"
        ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━Workspace━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
        ┃┌stack-1-branch-1─────────────────────┐┌stack-2-branch-1─────────────────────┐┃
        ┃│0 commits                            ││0 commits                            │┃
        ┃└─────────────────────────────────────┘└─────────────────────────────────────┘┃
        ┃╔stack-1-branch-2═════════════════════╗┌stack-2-branch-2─────────────────────┐┃
        ┃║0 commits                            ║│0 commits                            │┃
        ┃╚═════════════════════════════════════╝└─────────────────────────────────────┘┃
        ┃                                                                              ┃
        ┃                                                                              ┃
        ┗ Done <Enter> Cancel <Q> Select branch <S> Select all <A> Move with arrow keys┛
        ");

        // Move focus left, should wrap to second stack's head (stack-2-branch-1)
        selector.handle_key_event(event::KeyCode::Left.into());
        assert_eq!(selector.focused_branch, Some("stack-2-branch-1".into()));
        buf = Buffer::empty(Rect::new(0, 0, 80, 10));
        selector.render(buf.area, &mut buf);
        let buffer_str = buffer_to_string(&buf);
        insta::assert_snapshot!(buffer_str, @r"
        ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━Workspace━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
        ┃┌stack-1-branch-1─────────────────────┐╔stack-2-branch-1═════════════════════╗┃
        ┃│0 commits                            │║0 commits                            ║┃
        ┃└─────────────────────────────────────┘╚═════════════════════════════════════╝┃
        ┃┌stack-1-branch-2─────────────────────┐┌stack-2-branch-2─────────────────────┐┃
        ┃│0 commits                            ││0 commits                            │┃
        ┃└─────────────────────────────────────┘└─────────────────────────────────────┘┃
        ┃                                                                              ┃
        ┃                                                                              ┃
        ┗ Done <Enter> Cancel <Q> Select branch <S> Select all <A> Move with arrow keys┛
        ");

        // Move focus left again to first stack's head
        selector.handle_key_event(event::KeyCode::Left.into());
        assert_eq!(selector.focused_branch, Some("stack-1-branch-1".into()));
        buf = Buffer::empty(Rect::new(0, 0, 80, 10));
        selector.render(buf.area, &mut buf);
        let buffer_str = buffer_to_string(&buf);
        insta::assert_snapshot!(buffer_str, @r"
        ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━Workspace━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
        ┃╔stack-1-branch-1═════════════════════╗┌stack-2-branch-1─────────────────────┐┃
        ┃║0 commits                            ║│0 commits                            │┃
        ┃╚═════════════════════════════════════╝└─────────────────────────────────────┘┃
        ┃┌stack-1-branch-2─────────────────────┐┌stack-2-branch-2─────────────────────┐┃
        ┃│0 commits                            ││0 commits                            │┃
        ┃└─────────────────────────────────────┘└─────────────────────────────────────┘┃
        ┃                                                                              ┃
        ┃                                                                              ┃
        ┗ Done <Enter> Cancel <Q> Select branch <S> Select all <A> Move with arrow keys┛
        ");
    }
}

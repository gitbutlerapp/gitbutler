use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};

use super::*;

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
        stacks: vec![TuiStack {
            branches: vec![TuiBranch {
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
            TuiStack {
                branches: vec![TuiBranch {
                    name: "branch-1".into(),
                    ..Default::default()
                }],
            },
            TuiStack {
                branches: vec![TuiBranch {
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
        TuiCommit {
            title: "Add README".into(),
            sha: "d4e5f62".into(),
        },
        TuiCommit {
            title: "Initial commit".into(),
            sha: "a1b2c3d".into(),
        },
    ];

    let branch_2_commits = vec![
        TuiCommit {
            title: "Update the LICENSE".into(),
            sha: "3434344".into(),
        },
        TuiCommit {
            title: "Add LICENSE".into(),
            sha: "2222222".into(),
        },
    ];

    let selector = BranchSelectorState {
        stacks: vec![TuiStack {
            branches: vec![
                TuiBranch {
                    name: "branch-1".into(),
                    commits: branch_1_commits,
                    ..Default::default()
                },
                TuiBranch {
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
        stacks: vec![TuiStack {
            branches: vec![
                TuiBranch {
                    name: "branch-1".into(),
                    commits: vec![TuiCommit {
                        title: "First commit".into(),
                        sha: "abc1234".into(),
                    }],
                    ..Default::default()
                },
                TuiBranch {
                    name: "branch-2".into(),
                    commits: vec![TuiCommit {
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
        stacks: vec![TuiStack {
            branches: vec![
                TuiBranch {
                    name: "branch-1".into(),
                    commits: vec![TuiCommit {
                        title: "First commit".into(),
                        sha: "abc1234".into(),
                    }],
                    ..Default::default()
                },
                TuiBranch {
                    name: "branch-2".into(),
                    commits: vec![TuiCommit {
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
        stacks: vec![TuiStack {
            branches: vec![
                TuiBranch {
                    name: "branch-1".into(),
                    commits: vec![TuiCommit {
                        title: "First commit".into(),
                        sha: "abc1234".into(),
                    }],
                    ..Default::default()
                },
                TuiBranch {
                    name: "branch-2".into(),
                    commits: vec![TuiCommit {
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
        stacks: vec![TuiStack {
            branches: vec![
                TuiBranch {
                    name: "branch-1".into(),
                    commits: vec![],
                    ..Default::default()
                },
                TuiBranch {
                    name: "branch-2".into(),
                    commits: vec![],
                    ..Default::default()
                },
                TuiBranch {
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
            TuiStack {
                branches: vec![
                    TuiBranch {
                        name: "stack-1-branch-1".into(),
                        commits: vec![],
                        ..Default::default()
                    },
                    TuiBranch {
                        name: "stack-1-branch-2".into(),
                        commits: vec![],
                        ..Default::default()
                    },
                ],
            },
            TuiStack {
                branches: vec![
                    TuiBranch {
                        name: "stack-2-branch-1".into(),
                        commits: vec![],
                        ..Default::default()
                    },
                    TuiBranch {
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
            TuiStack {
                branches: vec![
                    TuiBranch {
                        name: "stack-1-branch-1".into(),
                        commits: vec![],
                        ..Default::default()
                    },
                    TuiBranch {
                        name: "stack-1-branch-2".into(),
                        commits: vec![],
                        ..Default::default()
                    },
                ],
            },
            TuiStack {
                branches: vec![
                    TuiBranch {
                        name: "stack-2-branch-1".into(),
                        commits: vec![],
                        ..Default::default()
                    },
                    TuiBranch {
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

mod utils {
    use ratatui::{buffer::Buffer, layout::Position};

    #[allow(dead_code)]
    /// Convert a ratatui Buffer into a readable string for snapshot testing
    pub fn buffer_to_string(buf: &Buffer) -> String {
        let mut output = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                let Some(cell) = buf.cell(Position::new(x, y)) else {
                    output.push(' ');
                    continue;
                };
                output.push(cell.symbol().chars().next().unwrap_or(' '));
            }
            output.push('\n');
        }
        output
    }
}
use utils::buffer_to_string;

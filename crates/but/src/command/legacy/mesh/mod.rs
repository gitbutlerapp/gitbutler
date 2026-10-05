//! `but mesh`: every machine and repository of a GitButler account at a glance, kept live by the
//! hosted server.
//!
//! It holds the account's connection to the server, which is also what shows this machine as
//! online, and loads what it shows in the background: the account's listing once, a repository's
//! branches as it's unfolded.

mod sign_in;
mod tree;

use std::{
    collections::{BTreeSet, HashMap, HashSet},
    io::Write as _,
    sync::mpsc,
    time::Duration,
};

use but_api::hosted::{
    FollowEvent, HostedAccountProject, HostedEvent, HostedProject, OnConflict, SyncOutcome,
};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph},
};
use tree::{Grouping, Load, LocalBranch, LocalRepo, Mesh, Row, RowKind, plural};

use crate::{
    args::mesh::Platform,
    tui::{CrosstermTerminalGuard, TerminalGuard},
};

/// What arrives from the background.
enum Update {
    /// `signed_out` when the server no longer knows the account's token.
    Account {
        account: Load<Vec<HostedAccountProject>>,
        signed_out: bool,
    },
    /// With the generation of the load, so one that a newer load overtook is dropped.
    Branches(String, u64, Load<Vec<LocalBranch>>),
    Hosted(String, u64, Load<HostedProject>),
    Server(HostedEvent),
    Acted(Action, anyhow::Result<SyncOutcome>),
    Followed(FollowEvent),
}

/// Something done to a branch, in the background as it talks to the hosted server. `path` is
/// the local repository it's done from.
#[derive(Clone)]
enum Action {
    Pull {
        path: String,
        machine: String,
        branch: String,
        into_workspace: bool,
        overwrite: bool,
    },
    Dismiss {
        path: String,
        machine: String,
        branch: String,
    },
    Publish {
        path: String,
        branch: String,
        include_uncommitted: bool,
    },
    Send {
        path: String,
        branch: String,
        to: String,
    },
}

impl Action {
    fn path(&self) -> &str {
        match self {
            Action::Pull { path, .. }
            | Action::Dismiss { path, .. }
            | Action::Publish { path, .. }
            | Action::Send { path, .. } => path,
        }
    }

    fn perform(self) -> anyhow::Result<SyncOutcome> {
        let mut ctx = but_ctx::Context::discover(self.path())?;
        Ok(match self {
            Action::Pull {
                machine,
                branch,
                into_workspace,
                overwrite,
                ..
            } => but_api::hosted::hosted_branch_pull(
                &mut ctx,
                machine,
                branch,
                into_workspace,
                overwrite.then_some(OnConflict::Overwrite),
            )?,
            Action::Dismiss {
                machine, branch, ..
            } => {
                but_api::hosted::hosted_branch_dismiss(&ctx, machine.clone(), branch.clone())?;
                SyncOutcome::Done(format!("Dismissed {branch} from {machine}"))
            }
            Action::Publish {
                branch,
                include_uncommitted,
                ..
            } => SyncOutcome::Done(but_api::hosted::hosted_branch_publish(
                &ctx,
                branch,
                include_uncommitted,
            )?),
            Action::Send { branch, to, .. } => SyncOutcome::Done(
                but_api::hosted::hosted_branch_send(&ctx, branch, to, false)?,
            ),
        })
    }
}

/// A question over the tree, answered before anything else.
enum Prompt {
    /// `y` does `yes`, `n` does `no` if there is one.
    Confirm {
        question: String,
        yes: Action,
        no: Option<Action>,
    },
    /// The mesh settings, with one of them chosen.
    Settings { selected: usize },
    /// Which machine to send `branch` to.
    SendTo {
        path: String,
        branch: String,
        machines: Vec<String>,
        selected: usize,
    },
}

pub fn run(args: Platform) -> anyhow::Result<()> {
    if args.sign_out {
        but_api::legacy::users::delete_user()?;
        println!("Signed out of GitButler");
        return Ok(());
    }
    let server = but_api::hosted::hosted_server();
    if args.headless {
        return headless(&server);
    }
    let mut terminal = CrosstermTerminalGuard::alt_screen(false)?;
    if but_api::legacy::users::get_user().ok().flatten().is_none()
        && !sign_in::run(&mut terminal, &server)?
    {
        return Ok(());
    }
    App::new(server)?.run(&mut terminal)
}

/// Stay online for the account and say what happens, without drawing anything.
fn headless(server: &str) -> anyhow::Result<()> {
    if but_api::legacy::users::get_user().ok().flatten().is_none() {
        anyhow::bail!("Not signed in to GitButler; run `but mesh` once to sign in");
    }
    let (tx, rx) = mpsc::channel();
    let _follower = but_api::hosted::follow(
        |event| println!("{}", event.message),
        move |event| {
            tx.send(event).ok();
        },
    )?;
    println!("Online at {server}, keeping branches in step; Ctrl-C to stop");
    let mut online = BTreeSet::new();
    for event in rx {
        match event {
            HostedEvent::Published(Some(root)) => {
                println!(
                    "published to the project rooted at {}",
                    &root[..root.len().min(12)]
                )
            }
            // A catch-up after (re)connecting, for listings to refresh; nothing to tell.
            HostedEvent::Published(None) => {}
            HostedEvent::Online(machines) => {
                let now: BTreeSet<_> = machines.into_iter().collect();
                for came in now.difference(&online) {
                    println!("{came} is online");
                }
                for left in online.difference(&now) {
                    println!("{left} went offline");
                }
                online = now;
            }
            // The follower says what came of it, pulled or not.
            HostedEvent::Sent(sent) => {
                println!("{} sent {} of {}", sent.from, sent.branch, sent.title)
            }
        }
    }
    Ok(())
}

struct App {
    server: String,
    mesh: Mesh,
    grouping: Grouping,
    unfolded: HashSet<String>,
    /// Data asked for, so a row needing it doesn't ask again every frame.
    requested: HashSet<String>,
    /// Each load's latest generation, by what's loaded.
    generations: HashMap<String, u64>,
    next_generation: u64,
    /// An account load in flight, and whether another was asked for meanwhile.
    account_loading: bool,
    account_wanted: bool,
    signed_out: bool,
    /// Where the cursor was, for when the row it was on goes away.
    last_index: usize,
    selected: Option<String>,
    toast: Option<String>,
    help: bool,
    prompt: Option<Prompt>,
    busy: Option<String>,
    tx: mpsc::Sender<Update>,
    rx: mpsc::Receiver<Update>,
}

impl App {
    fn new(server: String) -> anyhow::Result<Self> {
        let (tx, rx) = mpsc::channel();
        let mut app = App {
            server,
            mesh: Mesh {
                this_machine: but_api::hosted::machine_name(),
                locals: local_repos()?,
                account: None,
                branches: HashMap::new(),
                hosted: HashMap::new(),
                online: BTreeSet::new(),
            },
            grouping: Grouping::Machines,
            unfolded: HashSet::new(),
            requested: HashSet::new(),
            generations: HashMap::new(),
            next_generation: 0,
            account_loading: false,
            account_wanted: false,
            signed_out: false,
            last_index: 0,
            selected: None,
            toast: None,
            help: false,
            prompt: None,
            busy: None,
            tx,
            rx,
        };
        app.load_account();
        Ok(app)
    }

    fn run(mut self, terminal: &mut CrosstermTerminalGuard) -> anyhow::Result<()> {
        // Held for as long as the mesh is open: its connection is what shows this machine online.
        let (followed, hosted) = (self.tx.clone(), self.tx.clone());
        let _follower = but_api::hosted::follow(
            move |event| {
                followed.send(Update::Followed(event)).ok();
            },
            move |event| {
                hosted.send(Update::Server(event)).ok();
            },
        )?;
        loop {
            for update in self.rx.try_iter().collect::<Vec<_>>() {
                self.apply(update);
            }
            self.load_wanted();
            let rows = self.mesh.rows(self.grouping, &self.unfolded);
            let index = self.index_of(&rows);
            if let Some(index) = index {
                self.last_index = index;
            }
            if self.signed_out {
                self.signed_out = false;
                if !sign_in::run(terminal, &self.server)? {
                    return Ok(());
                }
                self.reload(None);
                continue;
            }
            terminal
                .terminal_mut()
                .draw(|frame| self.draw(frame, &rows, index))?;
            if !event::poll(Duration::from_millis(150))? {
                continue;
            }
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if key.kind != KeyEventKind::Press {
                continue;
            }
            if self.help {
                self.help = false;
                continue;
            }
            if let Some(prompt) = self.prompt.take() {
                self.answer(prompt, key.code);
                continue;
            }
            let row = index.and_then(|index| rows.get(index));
            match key.code {
                KeyCode::Char('q') => return Ok(()),
                KeyCode::Esc => self.toast = None,
                KeyCode::Char('?') => self.help = true,
                KeyCode::Char(',') => self.prompt = Some(Prompt::Settings { selected: 0 }),
                KeyCode::Up | KeyCode::Char('k') => self.select(&rows, index, -1),
                KeyCode::Down | KeyCode::Char('j') => self.select(&rows, index, 1),
                KeyCode::Left | KeyCode::Char('h') => {
                    if let Some(row) = row {
                        self.unfolded.remove(&row.key);
                    }
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    if let Some(row) = row.filter(|row| row.folded.is_some()) {
                        self.unfolded.insert(row.key.clone());
                    }
                }
                KeyCode::Enter | KeyCode::Char(' ') => {
                    if let Some(row) = row.filter(|row| row.folded.is_some())
                        && !self.unfolded.remove(&row.key)
                    {
                        self.unfolded.insert(row.key.clone());
                    }
                }
                KeyCode::Tab => {
                    self.grouping = match self.grouping {
                        Grouping::Machines => Grouping::Repos,
                        Grouping::Repos => Grouping::Machines,
                    }
                }
                KeyCode::Char('r') => self.reload(None),
                KeyCode::Char(c) => {
                    if let Some(row) = row {
                        self.act(row, c);
                    }
                }
                _ => {}
            }
        }
    }

    fn apply(&mut self, update: Update) {
        match update {
            Update::Account {
                account,
                signed_out,
            } => {
                self.mesh.account = Some(account);
                self.signed_out |= signed_out;
                self.account_loading = false;
                if std::mem::take(&mut self.account_wanted) {
                    self.load_account();
                }
            }
            Update::Branches(project, generation, branches) => {
                if self.generations.get(&format!("branches:{project}")) != Some(&generation) {
                    return;
                }
                self.mesh.branches.insert(project, branches);
            }
            Update::Hosted(project, generation, hosted) => {
                if self.generations.get(&format!("hosted:{project}")) != Some(&generation) {
                    return;
                }
                self.mesh.hosted.insert(project, hosted);
            }
            Update::Acted(action, outcome) => {
                self.busy = None;
                match outcome {
                    Ok(SyncOutcome::Done(done)) => self.toast = Some(done),
                    Ok(SyncOutcome::NeedsChoice(reason)) => {
                        if let Action::Pull { .. } = action {
                            let mut yes = action.clone();
                            if let Action::Pull { overwrite, .. } = &mut yes {
                                *overwrite = true;
                            }
                            self.prompt = Some(Prompt::Confirm {
                                question: format!("{reason} Overwrite?"),
                                yes,
                                no: None,
                            });
                        }
                    }
                    Err(err) => self.toast = Some(format!("Failed: {err:#}")),
                }
                self.reload_path(action.path());
            }
            Update::Followed(event) => {
                self.toast = Some(event.message);
                self.reload(None);
            }
            Update::Server(HostedEvent::Online(online)) => {
                self.mesh.online = online.into_iter().collect();
            }
            Update::Server(HostedEvent::Published(root)) => self.reload(root.as_deref()),
            Update::Server(HostedEvent::Sent(sent)) => {
                self.toast = Some(format!(
                    "{} sent you {} ({}) · Esc to dismiss",
                    sent.from, sent.branch, sent.title
                ));
                // The terminal's bell, so a send is noticed from another tab.
                std::io::stdout().write_all(b"\x07").ok();
                std::io::stdout().flush().ok();
                self.reload(Some(&sent.root));
            }
        }
    }

    /// Load again what a publish to `root` may have changed, or everything without one.
    fn reload(&mut self, root: Option<&str>) {
        let projects: Vec<String> = match (root, &self.mesh.account) {
            (Some(root), Some(Load::Loaded(account))) => account
                .iter()
                .filter(|entry| entry.root == root)
                .flat_map(|entry| entry.project_ids.clone())
                .collect(),
            _ => self
                .mesh
                .locals
                .iter()
                .map(|repo| repo.id.clone())
                .collect(),
        };
        // Its local branches too: what was published is often a branch here that just changed.
        for project in projects {
            self.requested.remove(&format!("hosted:{project}"));
            self.requested.remove(&format!("branches:{project}"));
        }
        self.load_account();
    }

    /// What a key does to `row`, if anything.
    fn act(&mut self, row: &Row, key: char) {
        match (&row.kind, key) {
            (
                RowKind::Branch {
                    name,
                    machine: Some(machine),
                    path,
                    ..
                },
                'p' | 'w',
            ) => self.perform(Action::Pull {
                path: path.clone(),
                machine: machine.clone(),
                branch: name.clone(),
                into_workspace: key == 'w',
                overwrite: false,
            }),
            (
                RowKind::Branch {
                    name,
                    machine: Some(machine),
                    path,
                    sent: true,
                    ..
                },
                'x',
            ) => self.perform(Action::Dismiss {
                path: path.clone(),
                machine: machine.clone(),
                branch: name.clone(),
            }),
            (
                RowKind::Branch {
                    name,
                    machine: None,
                    path,
                    worktree,
                    ..
                },
                'P',
            ) => {
                let publish = |include_uncommitted| Action::Publish {
                    path: path.clone(),
                    branch: name.clone(),
                    include_uncommitted,
                };
                // Uncommitted changes belong to a branch only in its own worktree.
                if worktree.is_some() {
                    self.prompt = Some(Prompt::Confirm {
                        question: format!("Publish {name} with its uncommitted changes?"),
                        yes: publish(true),
                        no: Some(publish(false)),
                    });
                } else {
                    self.perform(publish(false));
                }
            }
            (
                RowKind::Branch {
                    name,
                    machine: None,
                    path,
                    ..
                },
                's',
            ) => {
                let machines = self.mesh.machine_names();
                if machines.is_empty() {
                    self.toast = Some("No other machine to send to yet".into());
                } else {
                    self.prompt = Some(Prompt::SendTo {
                        path: path.clone(),
                        branch: name.clone(),
                        machines,
                        selected: 0,
                    });
                }
            }
            (
                RowKind::Repo {
                    path: Some(path), ..
                },
                'c',
            ) => {
                self.toast = Some(
                    match crate::tui::Clipboard::live().set_text(path.as_str()) {
                        Ok(()) => format!("Copied {path}"),
                        Err(err) => format!("Couldn't copy: {err:#}"),
                    },
                );
            }
            _ => {}
        }
    }

    fn answer(&mut self, prompt: Prompt, key: KeyCode) {
        match prompt {
            Prompt::Settings { selected } => {
                let selected = match key {
                    KeyCode::Up | KeyCode::Char('k') => selected.saturating_sub(1),
                    KeyCode::Down | KeyCode::Char('j') => (selected + 1).min(SETTINGS.len() - 1),
                    KeyCode::Esc | KeyCode::Char(',') | KeyCode::Char('q') => return,
                    KeyCode::Enter
                    | KeyCode::Char(' ')
                    | KeyCode::Left
                    | KeyCode::Right
                    | KeyCode::Char('h' | 'l' | '-' | '+') => {
                        let later = matches!(
                            key,
                            KeyCode::Right | KeyCode::Char('l' | '+' | ' ') | KeyCode::Enter
                        );
                        if let Err(err) = change_setting(selected, later) {
                            self.toast = Some(format!("Failed: {err:#}"));
                        }
                        selected
                    }
                    _ => selected,
                };
                self.prompt = Some(Prompt::Settings { selected });
            }
            Prompt::Confirm { yes, no, question } => match key {
                KeyCode::Char('y') => self.perform(yes),
                KeyCode::Char('n') => {
                    if let Some(no) = no {
                        self.perform(no);
                    }
                }
                KeyCode::Esc => {}
                // Anything else leaves the question up.
                _ => self.prompt = Some(Prompt::Confirm { question, yes, no }),
            },
            Prompt::SendTo {
                path,
                branch,
                machines,
                selected,
            } => {
                let selected = match key {
                    KeyCode::Up | KeyCode::Char('k') => selected.saturating_sub(1),
                    KeyCode::Down | KeyCode::Char('j') => (selected + 1).min(machines.len() - 1),
                    KeyCode::Enter => {
                        let to = machines[selected].clone();
                        return self.perform(Action::Send { path, branch, to });
                    }
                    KeyCode::Esc => return,
                    _ => selected,
                };
                self.prompt = Some(Prompt::SendTo {
                    path,
                    branch,
                    machines,
                    selected,
                });
            }
        }
    }

    fn perform(&mut self, action: Action) {
        self.toast = None;
        self.busy = Some("working…".into());
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let outcome = action.clone().perform();
            tx.send(Update::Acted(action, outcome)).ok();
        });
    }

    /// Load again the repository at `path`, which an action changed.
    fn reload_path(&mut self, path: &str) {
        if let Some(repo) = self.mesh.locals.iter().find(|repo| repo.path == path) {
            self.requested.remove(&format!("hosted:{}", repo.id));
            self.requested.remove(&format!("branches:{}", repo.id));
        }
        self.load_account();
    }

    /// Load the account's listing, once at a time: asked for while one is in flight, it's loaded
    /// again when that one is done.
    fn load_account(&mut self) {
        if self.account_loading {
            self.account_wanted = true;
            return;
        }
        self.account_loading = true;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let (account, signed_out) = match but_api::hosted::hosted_account() {
                Ok(account) => (Load::Loaded(account), false),
                Err(err) => (
                    Load::Failed(format!("{err:#}")),
                    but_api::hosted::is_signed_out(&err),
                ),
            };
            tx.send(Update::Account {
                account,
                signed_out,
            })
            .ok();
        });
    }

    /// Ask for whatever unfolded rows show and isn't loaded or asked for yet.
    fn load_wanted(&mut self) {
        let (local, hosted) = self.mesh.wanted(&self.unfolded);
        let mut jobs = Vec::new();
        for repo in local {
            if self.requested.insert(format!("branches:{}", repo.id)) {
                jobs.push((repo.id.clone(), repo.path.clone(), true));
            }
        }
        for repo in hosted {
            if self.requested.insert(format!("hosted:{}", repo.id)) {
                jobs.push((repo.id.clone(), repo.path.clone(), false));
            }
        }
        for (project, path, branches) in jobs {
            if branches {
                self.mesh
                    .branches
                    .entry(project.clone())
                    .or_insert(Load::Loading);
            } else {
                self.mesh
                    .hosted
                    .entry(project.clone())
                    .or_insert(Load::Loading);
            }
            self.next_generation += 1;
            let generation = self.next_generation;
            let what = if branches { "branches" } else { "hosted" };
            self.generations
                .insert(format!("{what}:{project}"), generation);
            let tx = self.tx.clone();
            std::thread::spawn(move || {
                let update = if branches {
                    Update::Branches(project, generation, load(local_branches(&path)))
                } else {
                    let hosted = but_ctx::Context::discover(&path)
                        .and_then(|ctx| but_api::hosted::hosted_machines(&ctx));
                    Update::Hosted(project, generation, load(hosted))
                };
                tx.send(update).ok();
            });
        }
    }

    fn index_of(&self, rows: &[Row]) -> Option<usize> {
        if rows.is_empty() {
            return None;
        }
        // Where it was when its row went away, rather than back at the top.
        Some(
            self.selected
                .as_ref()
                .and_then(|key| rows.iter().position(|row| &row.key == key))
                .unwrap_or(self.last_index.min(rows.len() - 1)),
        )
    }

    fn select(&mut self, rows: &[Row], index: Option<usize>, by: isize) {
        let Some(index) = index else { return };
        let next = index
            .saturating_add_signed(by)
            .min(rows.len().saturating_sub(1));
        self.selected = rows.get(next).map(|row| row.key.clone());
    }

    fn draw(&self, frame: &mut Frame, rows: &[Row], index: Option<usize>) {
        let dim = Style::default().add_modifier(Modifier::DIM);
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let [header, tabs, list, footer] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .areas(frame.area());

        frame.render_widget(
            Line::from(vec![
                Span::styled(" but mesh", bold),
                Span::styled(format!(" · {}", self.server), dim),
                Span::styled(
                    format!(
                        "   {}",
                        self.mesh.this_machine.as_deref().unwrap_or("this machine")
                    ),
                    dim,
                ),
            ]),
            header,
        );
        let tab = |label: &str, active: bool| {
            if active {
                Span::styled(format!(" [{label}] "), bold)
            } else {
                Span::styled(format!("  {label}  "), dim)
            }
        };
        frame.render_widget(
            Line::from(vec![
                tab("Machines", self.grouping == Grouping::Machines),
                tab("Repos", self.grouping == Grouping::Repos),
                Span::styled(
                    match &self.mesh.account {
                        Some(Load::Failed(err)) => format!("  {err}"),
                        Some(Load::Loading) | None => "  loading…".into(),
                        Some(Load::Loaded(_)) => String::new(),
                    },
                    dim,
                ),
            ]),
            tabs,
        );

        let items: Vec<ListItem> = rows
            .iter()
            .map(|row| ListItem::new(row_line(row)))
            .collect();
        let mut state = ListState::default().with_selected(index);
        frame.render_stateful_widget(
            List::new(items).highlight_style(Style::default().add_modifier(Modifier::REVERSED)),
            list,
            &mut state,
        );

        let selected = index.and_then(|index| rows.get(index));
        let hint = match (&self.prompt, &self.busy, &self.toast) {
            (Some(Prompt::Confirm { question, no, .. }), _, _) => Line::styled(
                format!(
                    " {question}  y yes · {}esc cancel",
                    if no.is_some() { "n no · " } else { "n/" }
                ),
                bold,
            ),
            (Some(Prompt::Settings { .. }), _, _) => {
                Line::styled(" ↑↓ choose · space toggle · ←→ change · esc close", bold)
            }
            (Some(Prompt::SendTo { branch, .. }), _, _) => Line::styled(
                format!(" Send {branch} to…  ↑↓ choose · enter send · esc cancel"),
                bold,
            ),
            (None, Some(busy), _) => Line::styled(format!(" {busy}"), dim),
            (None, None, Some(toast)) => Line::styled(format!(" {toast}"), bold),
            (None, None, None) => Line::styled(format!(" {}", hints(selected)), dim),
        };
        frame.render_widget(hint, footer);

        if let Some(Prompt::SendTo {
            machines, selected, ..
        }) = &self.prompt
        {
            let height = machines.len() as u16 + 2;
            let area = ratatui::layout::Rect {
                x: list.x + 4,
                y: list.y + list.height.saturating_sub(height),
                width: 32.min(list.width.saturating_sub(4)),
                height: height.min(list.height),
            };
            let items: Vec<ListItem> = machines
                .iter()
                .map(|machine| {
                    let online = if self.mesh.online.contains(machine) {
                        "● "
                    } else {
                        "○ "
                    };
                    ListItem::new(format!("{online}{machine}"))
                })
                .collect();
            let mut state = ListState::default().with_selected(Some(*selected));
            frame.render_widget(ratatui::widgets::Clear, area);
            frame.render_stateful_widget(
                List::new(items)
                    .block(ratatui::widgets::Block::bordered().title(" Send to "))
                    .highlight_style(Style::default().add_modifier(Modifier::REVERSED)),
                area,
                &mut state,
            );
        }

        if let Some(Prompt::Settings { selected }) = &self.prompt {
            let lines = settings_lines();
            let height = lines.len() as u16 + 2;
            let area = ratatui::layout::Rect {
                x: list.x + 4,
                y: list.y + 1,
                width: 54.min(list.width.saturating_sub(4)),
                height: height.min(list.height),
            };
            let items: Vec<ListItem> = lines.into_iter().map(ListItem::new).collect();
            let mut state = ListState::default().with_selected(Some(*selected));
            frame.render_widget(ratatui::widgets::Clear, area);
            frame.render_stateful_widget(
                List::new(items)
                    .block(ratatui::widgets::Block::bordered().title(" Mesh settings "))
                    .highlight_style(Style::default().add_modifier(Modifier::REVERSED)),
                area,
                &mut state,
            );
        }

        if self.help {
            let help = [
                "↑ ↓ / j k     move",
                "← → / h l     fold, unfold",
                "enter, space  fold or unfold",
                "tab           group by machines or repos",
                "r             refresh from the hosted server",
                ",             settings: keep published branches up to date, pull what's sent",
                "",
                "On another machine's branch:",
                "p / w         pull into a worktree / into the workspace",
                "x             dismiss, if it was sent to you",
                "On this machine's branch:",
                "P             publish",
                "s             send to another machine",
                "On a repository here:",
                "c             copy its path",
                "",
                "esc           dismiss a notice",
                "q             quit",
                "",
                "● online  ○ offline  ◍ published here (↑ ahead, ↓ behind, ! diverged)  ✉ sent to you",
                "",
                "Any key closes this.",
            ];
            let area = frame.area().inner(ratatui::layout::Margin::new(4, 3));
            frame.render_widget(ratatui::widgets::Clear, area);
            frame.render_widget(
                Paragraph::new(help.map(Line::from).to_vec())
                    .block(ratatui::widgets::Block::bordered().title(" Help ")),
                area,
            );
        }
    }
}

/// What the keys do on `row`, in a line.
fn hints(row: Option<&Row>) -> String {
    let what = match row.map(|row| &row.kind) {
        Some(RowKind::Branch {
            machine: Some(_),
            sent: true,
            ..
        }) => "p pull · w pull into workspace · x dismiss · ",
        Some(RowKind::Branch {
            machine: Some(_), ..
        }) => "p pull · w pull into workspace · ",
        Some(RowKind::Branch { machine: None, .. }) => "P publish · s send to · ",
        Some(RowKind::Repo { path: Some(_), .. }) => "c copy path · ",
        _ => "",
    };
    format!("{what}↑↓ move · ←→ fold · tab group · , settings · ? help · q quit")
}

fn row_line(row: &Row) -> Line<'static> {
    let dim = Style::default().add_modifier(Modifier::DIM);
    let mut spans = vec![Span::raw(format!(
        " {}{} ",
        "  ".repeat(row.depth),
        match row.folded {
            Some(true) => "▸",
            Some(false) => "▾",
            None => " ",
        }
    ))];
    match &row.kind {
        RowKind::Machine {
            name,
            this,
            online,
            published_at,
            sent,
        } => {
            spans.push(if *this {
                Span::raw("⌂ ")
            } else if *online {
                Span::styled("● ", Style::default().fg(Color::Green))
            } else {
                Span::styled("○ ", dim)
            });
            spans.push(Span::styled(
                name.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ));
            if *sent > 0 {
                spans.push(Span::styled(
                    format!("  ✉ {sent}"),
                    Style::default().fg(Color::Cyan),
                ));
            }
            if let Some(at) = published_at {
                spans.push(Span::styled(format!("  published {}", ago(*at)), dim));
            }
        }
        RowKind::Repo { title, detail, .. } => {
            spans.push(Span::raw(title.clone()));
            if !detail.is_empty() {
                spans.push(Span::styled(format!("  {detail}"), dim));
            }
        }
        RowKind::Branch {
            name,
            commits,
            worktree,
            publish,
            sent,
            ..
        } => {
            spans.push(Span::raw(match worktree {
                Some(worktree) => format!("▢ {worktree}/"),
                None => format!("⎇ {name}"),
            }));
            if worktree.as_deref().is_some_and(|worktree| worktree != name) {
                spans.push(Span::styled(format!(" {name}"), dim));
            }
            spans.push(Span::styled(
                format!("  {}", plural(*commits, "commit")),
                dim,
            ));
            if let Some(publish) = publish {
                use but_api::hosted::PublishState as P;
                spans.push(match publish {
                    P::Published => Span::styled("  ◍", dim),
                    P::Ahead(n) => Span::styled(format!("  ◍ ↑{n}"), dim),
                    P::Behind(n) => Span::styled(format!("  ◍ ↓{n}"), dim),
                    P::Diverged => Span::styled("  ◍ !", Style::default().fg(Color::Yellow)),
                });
            }
            if *sent {
                spans.push(Span::styled(
                    "  ✉ sent to you",
                    Style::default().fg(Color::Cyan),
                ));
            }
        }
        RowKind::Commit { title } => spans.push(Span::raw(format!("◦ {title}"))),
        RowKind::Note(text) => spans.push(Span::styled(text.clone(), dim)),
    }
    Line::from(spans)
}

/// How long ago `at`, in milliseconds since the Unix epoch, was.
fn ago(at: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |now| now.as_millis() as i64);
    let minutes = (now - at).max(0) / 60_000;
    match minutes {
        0 => "just now".into(),
        m if m < 60 => format!("{m}m ago"),
        m if m < 60 * 24 => format!("{}h ago", m / 60),
        m => format!("{}d ago", m / 60 / 24),
    }
}

fn load<T>(result: anyhow::Result<T>) -> Load<T> {
    match result {
        Ok(value) => Load::Loaded(value),
        Err(err) => Load::Failed(format!("{err:#}")),
    }
}

/// This machine's repositories, as GitButler lists them.
fn local_repos() -> anyhow::Result<Vec<LocalRepo>> {
    let projects = serde_json::to_value(but_api::legacy::projects::list_projects_stateless()?)?;
    let mut repos: Vec<LocalRepo> = projects
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|project| {
            Some(LocalRepo {
                id: project["id"].as_str()?.to_owned(),
                title: project["title"].as_str()?.to_owned(),
                path: project["path"].as_str()?.to_owned(),
            })
        })
        // A project whose directory is gone has nothing to show.
        .filter(|repo| std::path::Path::new(&repo.path).exists())
        .collect();
    repos.sort_by_key(|repo| repo.title.to_lowercase());
    Ok(repos)
}

/// A repository's branches, in its workspace and its linked worktrees.
fn local_branches(path: &str) -> anyhow::Result<Vec<LocalBranch>> {
    let ctx = but_ctx::Context::discover(path)?;
    let head = but_api::legacy::workspace::head_info(&ctx)?;
    let branch = |segment: &but_workspace::ref_info::Segment, worktree: Option<String>| {
        segment.ref_info.as_ref().map(|info| LocalBranch {
            name: info.ref_name.shorten().to_string(),
            worktree,
            commits: segment
                .commits
                .iter()
                .map(|commit| commit.inner.message.to_string())
                .collect(),
        })
    };
    let mut branches: Vec<LocalBranch> = head
        .stacks
        .iter()
        .flat_map(|stack| &stack.segments)
        .filter_map(|segment| branch(segment, None))
        .collect();
    for worktree in &head.worktrees {
        let name = worktree.name.to_string();
        branches.extend(
            worktree
                .segments
                .iter()
                .filter_map(|segment| branch(segment, Some(name.clone()))),
        );
    }
    Ok(branches)
}

/// The mesh settings, in the order the settings overlay lists them.
const SETTINGS: [&str; 3] = [
    "Keep published branches up to date",
    "  once changes settle for",
    "Pull branches sent here",
];
/// The waits a publish setting steps through, in seconds.
const SETTLE: [u64; 7] = [1, 2, 3, 5, 10, 30, 60];

fn mesh_settings() -> but_settings::app_settings::Mesh {
    but_settings::AppSettings::load_from_default_path_creating_without_customization()
        .map(|settings| settings.mesh)
        .unwrap_or_else(|_| but_settings::AppSettings::default().mesh)
}

fn settings_lines() -> Vec<String> {
    let mesh = mesh_settings();
    let on = |on: bool| if on { "on" } else { "off" };
    let every = |sec: u64| {
        if sec.is_multiple_of(60) && sec > 0 {
            format!("{} min", sec / 60)
        } else {
            format!("{sec} s")
        }
    };
    let values = [
        on(mesh.auto_publish).to_owned(),
        every(mesh.publish_after_sec),
        on(mesh.auto_pull).to_owned(),
    ];
    SETTINGS
        .iter()
        .zip(values)
        .map(|(name, value)| format!(" {name:<36}{value:>8}"))
        .collect()
}

/// Turn the setting at `index` on or off, or its interval up (`later`) or down.
fn change_setting(index: usize, later: bool) -> anyhow::Result<()> {
    let mesh = mesh_settings();
    let step = |steps: &[u64], sec: u64| -> u64 {
        let position = steps
            .iter()
            .position(|&i| i >= sec)
            .unwrap_or(steps.len() - 1);
        let position = if later {
            (position + 1).min(steps.len() - 1)
        } else {
            position.saturating_sub(1)
        };
        steps[position]
    };
    let mut update = but_settings::api::MeshUpdate::default();
    match index {
        0 => update.auto_publish = Some(!mesh.auto_publish),
        1 => update.publish_after_sec = Some(step(&SETTLE, mesh.publish_after_sec)),
        _ => update.auto_pull = Some(!mesh.auto_pull),
    }
    let config_dir = but_path::app_config_dir()?;
    but_settings::AppSettingsWithDiskSync::new_with_customization(config_dir, None)?
        .update_mesh(update)
}

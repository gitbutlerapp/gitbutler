//! What `but mesh` shows, and the rows it draws from it.

use std::collections::{BTreeSet, HashMap, HashSet};

use but_api::hosted::{HostedAccountProject, HostedProject, PublishState};

/// Something loaded in the background.
pub(super) enum Load<T> {
    Loading,
    Loaded(T),
    Failed(String),
}

/// A repository on this machine, as GitButler knows it.
pub(super) struct LocalRepo {
    pub id: String,
    pub title: String,
    pub path: String,
}

/// A branch of this machine's, in the workspace or a linked worktree.
pub(super) struct LocalBranch {
    pub name: String,
    pub worktree: Option<String>,
    pub commits: Vec<String>,
}

/// Everything `but mesh` knows, as it arrives.
pub(super) struct Mesh {
    pub this_machine: Option<String>,
    pub locals: Vec<LocalRepo>,
    pub account: Option<Load<Vec<HostedAccountProject>>>,
    /// Per local project: its branches, then what the hosted server has of it.
    pub branches: HashMap<String, Load<Vec<LocalBranch>>>,
    pub hosted: HashMap<String, Load<HostedProject>>,
    pub online: BTreeSet<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Grouping {
    Machines,
    Repos,
}

/// One line of the tree.
pub(super) struct Row {
    pub key: String,
    pub depth: usize,
    /// `Some` where the row folds, and whether it's folded.
    pub folded: Option<bool>,
    pub kind: RowKind,
}

pub(super) enum RowKind {
    Machine {
        name: String,
        this: bool,
        online: bool,
        published_at: Option<i64>,
        /// Its branches waiting in this machine's inbox.
        sent: usize,
    },
    Repo {
        title: String,
        detail: String,
        /// Where this machine has it, if it does.
        path: Option<String>,
    },
    Branch {
        name: String,
        commits: usize,
        worktree: Option<String>,
        publish: Option<PublishState>,
        sent: bool,
        /// Whose it is: `None` for this machine's.
        machine: Option<String>,
        /// The local repository it's acted on from.
        path: String,
    },
    Commit {
        title: String,
    },
    Note(String),
}

const THIS: &str = "this";

impl Mesh {
    /// The account listing's entry for a local project, if it has one.
    fn entry_for(&self, project: &str) -> Option<&HostedAccountProject> {
        match &self.account {
            Some(Load::Loaded(account)) => account
                .iter()
                .find(|entry| entry.project_ids.iter().any(|id| id == project)),
            _ => None,
        }
    }

    fn path_of(&self, project: &str) -> Option<String> {
        self.locals
            .iter()
            .find(|repo| repo.id == project)
            .map(|repo| repo.path.clone())
    }

    /// Other machines, by name: those that published, and those online.
    pub fn machine_names(&self) -> Vec<String> {
        let mut names: BTreeSet<String> = self.online.clone();
        if let Some(Load::Loaded(account)) = &self.account {
            for entry in account {
                names.extend(entry.machines.iter().map(|machine| machine.name.clone()));
            }
        }
        names.into_iter().collect()
    }

    /// The projects whose data the unfolded rows need: (local branches, hosted data).
    pub fn wanted(&self, unfolded: &HashSet<String>) -> (Vec<&LocalRepo>, Vec<&LocalRepo>) {
        let mut local = Vec::new();
        let mut hosted = Vec::new();
        for repo in &self.locals {
            let here =
                unfolded.contains(&local_key(&repo.id)) || unfolded.contains(&repo_key(&repo.id));
            if here {
                local.push(repo);
                hosted.push(repo);
            } else if unfolded
                .iter()
                .any(|key| key.starts_with("remote:") && key.ends_with(&format!(":{}", repo.id)))
            {
                hosted.push(repo);
            }
        }
        (local, hosted)
    }

    pub fn rows(&self, grouping: Grouping, unfolded: &HashSet<String>) -> Vec<Row> {
        let mut rows = Vec::new();
        match grouping {
            Grouping::Machines => {
                let key = machine_key(THIS);
                let folded = !unfolded.contains(&key);
                rows.push(Row {
                    key,
                    depth: 0,
                    folded: Some(folded),
                    kind: RowKind::Machine {
                        name: "This machine".into(),
                        this: true,
                        online: true,
                        published_at: None,
                        sent: 0,
                    },
                });
                if !folded {
                    for repo in &self.locals {
                        self.push_local_repo(&mut rows, repo, 1, &local_key(&repo.id), unfolded);
                    }
                }
                for name in self.machine_names() {
                    self.push_machine(&mut rows, &name, unfolded);
                }
            }
            Grouping::Repos => {
                for repo in &self.locals {
                    let key = repo_key(&repo.id);
                    let folded = !unfolded.contains(&key);
                    let machines = self.entry_for(&repo.id).map(|entry| entry.machines.len());
                    rows.push(Row {
                        key: key.clone(),
                        depth: 0,
                        folded: Some(folded),
                        kind: RowKind::Repo {
                            title: repo.title.clone(),
                            detail: with_sent(
                                match machines {
                                    Some(n) if n > 0 => {
                                        format!("here and on {}", plural(n, "machine"))
                                    }
                                    _ => String::new(),
                                },
                                self.entry_for(&repo.id).map_or(0, |entry| {
                                    entry.machines.iter().map(|m| m.sent as usize).sum()
                                }),
                            ),
                            path: Some(repo.path.clone()),
                        },
                    });
                    if folded {
                        continue;
                    }
                    self.push_local_branches(&mut rows, repo, 1, unfolded);
                    if let Some(entry) = self.entry_for(&repo.id) {
                        for machine in &entry.machines {
                            let key = remote_key(&machine.name, &repo.id);
                            let folded = !unfolded.contains(&key);
                            rows.push(Row {
                                key,
                                depth: 1,
                                folded: Some(folded),
                                kind: RowKind::Machine {
                                    name: machine.name.clone(),
                                    this: false,
                                    online: self.online.contains(&machine.name),
                                    published_at: Some(machine.published_at),
                                    sent: machine.sent as usize,
                                },
                            });
                            if !folded {
                                self.push_remote_branches(
                                    &mut rows,
                                    &machine.name,
                                    &repo.id,
                                    2,
                                    unfolded,
                                );
                            }
                        }
                    }
                }
                if let Some(Load::Loaded(account)) = &self.account {
                    for entry in account.iter().filter(|entry| entry.project_ids.is_empty()) {
                        rows.push(Row {
                            key: format!("hub:{}", entry.root),
                            depth: 0,
                            folded: None,
                            kind: RowKind::Repo {
                                title: entry.title.clone(),
                                detail: "not on this machine".into(),
                                path: None,
                            },
                        });
                    }
                }
            }
        }
        rows
    }

    fn push_machine(&self, rows: &mut Vec<Row>, name: &str, unfolded: &HashSet<String>) {
        let key = machine_key(name);
        let folded = !unfolded.contains(&key);
        let published: Vec<&HostedAccountProject> = match &self.account {
            Some(Load::Loaded(account)) => account
                .iter()
                .filter(|entry| entry.machines.iter().any(|machine| machine.name == name))
                .collect(),
            _ => Vec::new(),
        };
        let published_at = published
            .iter()
            .flat_map(|entry| entry.machines.iter())
            .filter(|machine| machine.name == name)
            .map(|machine| machine.published_at)
            .max();
        let sent = published
            .iter()
            .flat_map(|entry| entry.machines.iter())
            .filter(|machine| machine.name == name)
            .map(|machine| machine.sent as usize)
            .sum();
        rows.push(Row {
            key,
            depth: 0,
            folded: Some(folded),
            kind: RowKind::Machine {
                name: name.to_owned(),
                this: false,
                online: self.online.contains(name),
                published_at,
                sent,
            },
        });
        if folded {
            return;
        }
        if published.is_empty() {
            rows.push(note(rows.len(), 1, "Nothing published yet"));
        }
        for entry in published {
            let summary = entry.machines.iter().find(|machine| machine.name == name);
            let count = summary.map_or(0, |machine| machine.branches as usize);
            let sent = summary.map_or(0, |machine| machine.sent as usize);
            let Some(project) = entry.project_ids.first() else {
                rows.push(Row {
                    key: format!("hub:{name}:{}", entry.root),
                    depth: 1,
                    folded: None,
                    kind: RowKind::Repo {
                        title: entry.title.clone(),
                        detail: "not on this machine".into(),
                        path: None,
                    },
                });
                continue;
            };
            let key = remote_key(name, project);
            let folded = !unfolded.contains(&key);
            rows.push(Row {
                key,
                depth: 1,
                folded: Some(folded),
                kind: RowKind::Repo {
                    title: entry.title.clone(),
                    detail: with_sent(plural(count, "branch"), sent),
                    path: self.path_of(project),
                },
            });
            if !folded {
                self.push_remote_branches(rows, name, project, 2, unfolded);
            }
        }
    }

    fn push_local_repo(
        &self,
        rows: &mut Vec<Row>,
        repo: &LocalRepo,
        depth: usize,
        key: &str,
        unfolded: &HashSet<String>,
    ) {
        let folded = !unfolded.contains(key);
        rows.push(Row {
            key: key.to_owned(),
            depth,
            folded: Some(folded),
            kind: RowKind::Repo {
                title: repo.title.clone(),
                detail: String::new(),
                path: Some(repo.path.clone()),
            },
        });
        if !folded {
            self.push_local_branches(rows, repo, depth + 1, unfolded);
        }
    }

    fn push_local_branches(
        &self,
        rows: &mut Vec<Row>,
        repo: &LocalRepo,
        depth: usize,
        unfolded: &HashSet<String>,
    ) {
        let branches = match self.branches.get(&repo.id) {
            None | Some(Load::Loading) => return rows.push(note(rows.len(), depth, "Loading…")),
            Some(Load::Failed(err)) => return rows.push(note(rows.len(), depth, err)),
            Some(Load::Loaded(branches)) => branches,
        };
        if branches.is_empty() {
            rows.push(note(rows.len(), depth, "No branches"));
        }
        let hosted = match self.hosted.get(&repo.id) {
            Some(Load::Loaded(hosted)) => Some(hosted),
            _ => None,
        };
        let published: HashMap<&str, &PublishState> = hosted
            .iter()
            .flat_map(|hosted| &hosted.published_here)
            .map(|published| (published.branch.as_str(), &published.state))
            .collect();
        for branch in branches {
            let key = format!("branch:{}:{}", repo.id, branch.name);
            let folded = !unfolded.contains(&key);
            rows.push(Row {
                key: key.clone(),
                depth,
                folded: (!branch.commits.is_empty()).then_some(folded),
                kind: RowKind::Branch {
                    name: branch.name.clone(),
                    commits: branch.commits.len(),
                    worktree: branch.worktree.clone(),
                    publish: published
                        .get(branch.name.as_str())
                        .map(|state| (*state).clone()),
                    sent: false,
                    machine: None,
                    path: repo.path.clone(),
                },
            });
            if !folded {
                push_commits(rows, &key, depth + 1, branch.commits.iter().cloned());
            }
        }
    }

    fn push_remote_branches(
        &self,
        rows: &mut Vec<Row>,
        machine: &str,
        project: &str,
        depth: usize,
        unfolded: &HashSet<String>,
    ) {
        let hosted = match self.hosted.get(project) {
            None | Some(Load::Loading) => return rows.push(note(rows.len(), depth, "Loading…")),
            Some(Load::Failed(err)) => return rows.push(note(rows.len(), depth, err)),
            Some(Load::Loaded(hosted)) => hosted,
        };
        let Some(published) = hosted
            .machines
            .iter()
            .find(|candidate| candidate.name == machine)
        else {
            return rows.push(note(rows.len(), depth, "Nothing published"));
        };
        for branch in &published.branches {
            let key = format!("remote-branch:{machine}:{project}:{}", branch.branch);
            let folded = !unfolded.contains(&key);
            rows.push(Row {
                key: key.clone(),
                depth,
                folded: (!branch.commits.is_empty()).then_some(folded),
                kind: RowKind::Branch {
                    name: branch.branch.clone(),
                    commits: branch.commits.len(),
                    worktree: None,
                    publish: None,
                    sent: branch.sent,
                    machine: Some(machine.to_owned()),
                    path: self.path_of(project).unwrap_or_default(),
                },
            });
            if !folded {
                let titles = branch
                    .commits
                    .iter()
                    .map(|commit| commit.message.to_string());
                push_commits(rows, &key, depth + 1, titles);
            }
        }
    }
}

fn push_commits(
    rows: &mut Vec<Row>,
    parent: &str,
    depth: usize,
    messages: impl Iterator<Item = String>,
) {
    for (index, message) in messages.enumerate() {
        rows.push(Row {
            key: format!("{parent}:commit:{index}"),
            depth,
            folded: None,
            kind: RowKind::Commit {
                title: message.lines().next().unwrap_or_default().to_owned(),
            },
        });
    }
}

/// A line of text at `position` in the rows, which keeps its key unique: the same text, such as
/// "Loading…", shows under many rows at once.
fn note(position: usize, depth: usize, text: &str) -> Row {
    Row {
        key: format!("note:{position}:{text}"),
        depth,
        folded: None,
        kind: RowKind::Note(text.to_owned()),
    }
}

/// `detail`, followed by how many branches wait to be pulled here, if any.
fn with_sent(detail: String, sent: usize) -> String {
    match (sent, detail.is_empty()) {
        (0, _) => detail,
        (_, true) => format!("✉ {sent} sent to you"),
        (_, false) => format!("{detail} · ✉ {sent} sent to you"),
    }
}

pub(super) fn plural(count: usize, noun: &str) -> String {
    match (count, noun) {
        (1, _) => format!("1 {noun}"),
        (_, "branch") => format!("{count} branches"),
        _ => format!("{count} {noun}s"),
    }
}

fn machine_key(name: &str) -> String {
    format!("machine:{name}")
}

fn local_key(project: &str) -> String {
    format!("local:{project}")
}

fn repo_key(project: &str) -> String {
    format!("repo:{project}")
}

fn remote_key(machine: &str, project: &str) -> String {
    format!("remote:{machine}:{project}")
}

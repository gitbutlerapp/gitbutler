use std::collections::HashSet;

use gpui::{AnyElement, App, ClickEvent, Context, Toggled, Window, div, prelude::*};

use super::{Example, example, page, section};
use crate::theme::Theme;
use crate::ui::{FileStatus, dp, file_list, file_list_item, tree_guides};

/// The stories of `FileList.stories.tsx` this client can draw, Discarding at rest, before its
/// Discard. `Virtualised` and `Filter` are the list's in a browser: GPUI's lists scroll their own
/// way, and the filter is the app's.
pub(super) const EXAMPLES: [Example; 4] = [
    Example {
        id: "components-filelist--default",
        name: "Default",
        draw: |theme| {
            sized(file_list(
                "Changes",
                1,
                (12, 3),
                None,
                [file_list_item("default", "userSlice.ts")
                    .directory("src/store")
                    .status(FileStatus::Modification)
                    .line_counts(12, 3)
                    .into_any_element()],
                theme,
            ))
        },
    },
    Example {
        id: "components-filelist--states",
        name: "States",
        draw: |theme| {
            let focused = file_list(
                "Changes",
                5,
                (120, 48),
                None,
                [
                    file_list_item("added", "useAuthHook.ts")
                        .directory("src/hooks")
                        .status(FileStatus::Addition)
                        .line_counts(64, 0),
                    file_list_item("selected", "Button.tsx")
                        .directory("src/components")
                        .status(FileStatus::Modification)
                        .line_counts(18, 9)
                        .selected(true)
                        .focused(true),
                    file_list_item("reviewed", "apiService.ts")
                        .directory("src/services")
                        .status(FileStatus::Modification)
                        .line_counts(4, 2)
                        .reviewed(true),
                    file_list_item("conflicted", "merge.rs")
                        .directory("crates/core")
                        .status(FileStatus::Modification)
                        .line_counts(34, 37)
                        .conflicted(true),
                    file_list_item("inert", "index.ts")
                        .directory("src")
                        .status(FileStatus::Deletion)
                        .inert(true),
                ]
                .map(IntoElement::into_any_element),
                theme,
            );
            let unfocused = file_list(
                "Changes",
                2,
                (30, 0),
                None,
                [
                    file_list_item("styles", "styles.css")
                        .directory("src/styles")
                        .status(FileStatus::Addition),
                    file_list_item("theme", "theme.css")
                        .directory("src/styles")
                        .status(FileStatus::Modification)
                        .selected(true),
                ]
                .map(IntoElement::into_any_element),
                theme,
            );
            sized(
                div()
                    .flex()
                    .flex_col()
                    .gap(dp(24.))
                    .child(focused)
                    .child(unfocused),
            )
        },
    },
    Example {
        id: "components-filelist--discarding",
        name: "Discarding",
        draw: |theme| {
            // At rest, before Discard: the three picked files as one block.
            let picked = |id, name, directory, joined: (bool, bool)| {
                file_list_item(id, name)
                    .directory(directory)
                    .status(FileStatus::Modification)
                    .selected(true)
                    .focused(true)
                    .joined(joined.0, joined.1)
            };
            sized(file_list(
                "Changes",
                5,
                (0, 0),
                None,
                [
                    file_list_item("hook", "useAuthHook.ts")
                        .directory("src/hooks")
                        .status(FileStatus::Addition),
                    picked("button", "Button.tsx", "src/components", (false, true)),
                    picked("api", "apiService.ts", "src/services", (true, true)),
                    picked("routes", "routes.ts", "src", (true, false)),
                    file_list_item("theme", "theme.css")
                        .directory("src/styles")
                        .status(FileStatus::Modification),
                ]
                .map(IntoElement::into_any_element),
                theme,
            ))
        },
    },
    Example {
        id: "components-filelist--tree",
        name: "Tree",
        draw: |theme| {
            let folded = HashSet::from(["src/styles"]);
            let checked = HashSet::from(["src/components/Button.tsx"]);
            sized(tree(theme, &folded, &checked, |_| None, |_, _| None))
        },
    },
];

/// As wide as the docs site's stories are, 340px.
fn sized(list: impl IntoElement) -> AnyElement {
    div().w(dp(340.)).child(list).into_any_element()
}

enum Node {
    Dir(&'static str, Vec<Node>),
    File(&'static str, FileStatus),
}

/// The Tree story's files.
fn files() -> Vec<Node> {
    use FileStatus::*;
    vec![
        Node::Dir(
            "src",
            vec![
                Node::Dir(
                    "components",
                    vec![
                        Node::File("Button.tsx", Modification),
                        Node::File("Tooltip.tsx", Addition),
                    ],
                ),
                Node::Dir(
                    "styles",
                    vec![
                        Node::File("theme.css", Modification),
                        Node::File("reset.css", Deletion),
                        Node::File("tokens.css", Addition),
                    ],
                ),
                Node::File("index.ts", Modification),
            ],
        ),
        Node::File("package.json", Modification),
    ]
}

/// The paths of the files under `node`, at `path`.
fn file_paths(node: &Node, path: &str) -> Vec<String> {
    match node {
        Node::File(..) => vec![path.to_owned()],
        Node::Dir(_, children) => (children.iter())
            .flat_map(|child| file_paths(child, &format!("{path}/{}", child.name())))
            .collect(),
    }
}

impl Node {
    fn name(&self) -> &'static str {
        match self {
            Node::Dir(name, _) | Node::File(name, _) => name,
        }
    }
}

type Handler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// The tree as it shows with `folded` folded and `checked` checked: `on_fold` and `on_check` give
/// a directory's chevron and a row's checkbox what to do, by path, and the files a directory's
/// checkbox stands for.
fn tree(
    theme: &Theme,
    folded: &HashSet<&str>,
    checked: &HashSet<&str>,
    on_fold: impl Fn(String) -> Option<Handler>,
    on_check: impl Fn(Vec<String>, bool) -> Option<Handler>,
) -> AnyElement {
    let mut shown: Vec<(&Node, usize, String)> = Vec::new();
    fn visit<'a>(
        nodes: &'a [Node],
        depth: usize,
        parent: &str,
        folded: &HashSet<&str>,
        shown: &mut Vec<(&'a Node, usize, String)>,
    ) {
        for node in nodes {
            let path = match parent {
                "" => node.name().to_owned(),
                parent => format!("{parent}/{}", node.name()),
            };
            shown.push((node, depth, path.clone()));
            if let Node::Dir(_, children) = node
                && !folded.contains(path.as_str())
            {
                visit(children, depth + 1, &path, folded, shown);
            }
        }
    }
    let nodes = files();
    visit(&nodes, 0, "", folded, &mut shown);
    let depths: Vec<usize> = shown.iter().map(|(_, depth, _)| *depth).collect();
    let rows = (shown.iter().zip(tree_guides(&depths)).enumerate()).map(
        |(ix, ((node, depth, path), guides))| {
            let files = file_paths(node, path);
            let done = files
                .iter()
                .filter(|f| checked.contains(f.as_str()))
                .count();
            let check = match done {
                0 => Toggled::False,
                n if n == files.len() => Toggled::True,
                _ => Toggled::Mixed,
            };
            let row = file_list_item(("row", ix), node.name()).depth(*depth, guides);
            let row = match on_check(files.clone(), done < files.len()) {
                Some(handler) => row.checkbox(check, handler),
                None => row.checkbox(check, |_, _, _| {}),
            };
            match node {
                Node::File(_, status) => row.status(*status),
                Node::Dir(..) => {
                    let is_folded = folded.contains(path.as_str());
                    let row = row.folded(is_folded);
                    let row = match is_folded {
                        true => row.count(files.len()),
                        false => row,
                    };
                    match on_fold(path.clone()) {
                        Some(handler) => row.on_toggle_folded(handler),
                        None => row.on_toggle_folded(|_, _, _| {}),
                    }
                }
            }
            .into_any_element()
        },
    );
    file_list("Changes", 6, (214, 37), None, rows, theme)
        .w(dp(340.))
        .into_any_element()
}

/// FileList's stories, then the tree to fold and check.
pub struct FileListStory {
    folded: HashSet<String>,
    checked: HashSet<String>,
}

impl Default for FileListStory {
    fn default() -> Self {
        Self {
            folded: HashSet::from(["src/styles".to_owned()]),
            checked: HashSet::from(["src/components/Button.tsx".to_owned()]),
        }
    }
}

impl Render for FileListStory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx);
        let folded: HashSet<&str> = self.folded.iter().map(String::as_str).collect();
        let checked: HashSet<&str> = self.checked.iter().map(String::as_str).collect();
        let live = tree(
            theme,
            &folded,
            &checked,
            |path| {
                Some(Box::new(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    if !this.folded.remove(&path) {
                        this.folded.insert(path.clone());
                    }
                    cx.notify();
                })))
            },
            |files, on| {
                Some(Box::new(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    for file in &files {
                        match on {
                            true => this.checked.insert(file.clone()),
                            false => this.checked.remove(file),
                        };
                    }
                    cx.notify();
                })))
            },
        );
        page(
            (EXAMPLES.into_iter().map(|e| example(e, theme))).chain([section(
                "Tree, to fold and check",
                theme,
                [live],
            )]),
        )
    }
}

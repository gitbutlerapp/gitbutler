use std::collections::HashSet;
use std::ops::Range;

use gpui::{
    AnyElement, App, ClickEvent, Context, Entity, Focusable as _, Toggled, Window, div, prelude::*,
    uniform_list,
};

use super::{Example, example, page, section};
use crate::theme::Theme;
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{FileStatus, Icon, dp, file_list, file_list_item, fixed, icon_button, tree_guides};

/// The stories of `FileList.stories.tsx`, Discarding at rest, before its Discard, and Filter open
/// on "comp", as it starts; the live filter after them takes typing.
pub(super) const EXAMPLES: [Example; 7] = [
    Example {
        id: "components-filelist--default",
        name: "Default",
        draw: |_| {
            sized(
                file_list("default", "Changes", 1, (12, 3)).child(
                    file_list_item("default", "userSlice.ts")
                        .directory("src/store")
                        .status(FileStatus::Modification)
                        .line_counts(12, 3),
                ),
            )
        },
    },
    Example {
        id: "components-filelist--states",
        name: "States",
        draw: |_| {
            let focused = file_list("focused", "Changes", 5, (120, 48)).children(
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
            );
            let unfocused = file_list("unfocused", "Changes", 3, (30, 0)).children(
                [
                    file_list_item("styles", "styles.css")
                        .directory("src/styles")
                        .status(FileStatus::Addition),
                    file_list_item("theme", "theme.css")
                        .directory("src/styles")
                        .status(FileStatus::Modification)
                        .selected(true),
                    file_list_item("tokens", "tokens.css")
                        .directory("src/styles")
                        .editing(true),
                ]
                .map(IntoElement::into_any_element),
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
        draw: |_| {
            // At rest, before Discard: the three picked files as one block.
            let picked = |id, name, directory, joined: (bool, bool)| {
                file_list_item(id, name)
                    .directory(directory)
                    .status(FileStatus::Modification)
                    .selected(true)
                    .focused(true)
                    .joined(joined.0, joined.1)
            };
            sized(
                file_list("discarding", "Changes", 5, (0, 0)).children(
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
                ),
            )
        },
    },
    Example {
        id: "components-filelist--tree",
        name: "Tree",
        draw: |_| {
            let folded = HashSet::from(["src/styles"]);
            let checked = HashSet::from(["src/components/Button.tsx"]);
            sized(tree("tree", &folded, &checked, |_| None, |_, _| None))
        },
    },
    Example {
        id: "components-filelist--virtualised",
        name: "Virtualised",
        draw: |_| {
            // 360px tall, as the story's frame; each row ends on the pixel between rows.
            let rows = uniform_list("many", MANY, |range, _, _| {
                range
                    .map(|ix| div().pb(fixed(1.)).child(many(ix, ix == 0)))
                    .collect()
            });
            sized(
                div().flex().flex_col().h(dp(360.)).child(
                    file_list("virtualised", "Changes", MANY, (48210, 9021))
                        .flex_grow(1.)
                        .min_h_0()
                        .virtualised(rows),
                ),
            )
        },
    },
    Example {
        id: "components-filelist--filter",
        name: "Filter",
        draw: |theme| {
            let query = div().text_color(theme.foreground).child("comp");
            sized(
                filtered("filter", Some("comp"))
                    .filter(query, true, |_, _, _| {})
                    .found(2),
            )
        },
    },
    Example {
        id: "components-filelist--long-paths",
        name: "Long Paths",
        draw: |_| {
            let item = |id, name, directory, status, (added, removed)| {
                file_list_item(id, name)
                    .directory(directory)
                    .status(status)
                    .line_counts(added, removed)
            };
            div()
                .w(dp(300.))
                .child(file_list("long-paths", "Changes", 3, (21, 4)).children([
                    item(
                        "upstream",
                        "mod.rs",
                        "crates/but-gpui/src/detail/upstream",
                        FileStatus::Modification,
                        (12, 3),
                    ),
                    item(
                        "graph",
                        "mod.rs",
                        "crates/but-gpui/src/detail/graph",
                        FileStatus::Modification,
                        (8, 1),
                    ),
                    item(
                        "text-field",
                        "TextField.tsx",
                        "packages/design-system/components/forms/inputs",
                        FileStatus::Addition,
                        (1, 0),
                    ),
                ]))
                .into_any_element()
        },
    },
];

/// The Virtualised story's 2000 files, twenty to a folder.
const MANY: usize = 2000;

fn many(ix: usize, selected: bool) -> AnyElement {
    let status = match ix % 4 {
        0 => FileStatus::Addition,
        1 => FileStatus::Deletion,
        2 => FileStatus::Modification,
        _ => FileStatus::Rename,
    };
    file_list_item(("many", ix), format!("file-{ix}.ts"))
        .directory(format!("src/module-{}", ix / 20))
        .status(status)
        .selected(selected)
        .focused(true)
        .into_any_element()
}

/// The Filter story's files.
const FILTERABLE: [(&str, &str, FileStatus); 5] = [
    ("useAuthHook.ts", "src/hooks", FileStatus::Addition),
    ("Button.tsx", "src/components", FileStatus::Modification),
    ("Tooltip.tsx", "src/components", FileStatus::Modification),
    ("theme.css", "src/styles", FileStatus::Modification),
    ("index.ts", "src", FileStatus::Deletion),
];

/// Every place `query` is in `text`, ignoring case.
fn found(text: &str, query: &str) -> Vec<Range<usize>> {
    if query.is_empty() {
        return Vec::new();
    }
    let (text, query) = (text.to_lowercase(), query.to_lowercase());
    (text.match_indices(&query))
        .map(|(at, _)| at..at + query.len())
        .collect()
}

/// The Filter story's list with `query` open, its files narrowed to those it finds and what it
/// found in each underlined, or with the filter shut.
fn filtered(id: &'static str, query: Option<&str>) -> crate::ui::FileList {
    let rows = (FILTERABLE.iter())
        .filter(|(name, directory, _)| {
            query.is_none_or(|query| format!("{directory}/{name}").contains(query))
        })
        .map(|(name, directory, status)| {
            let query = query.unwrap_or_default();
            file_list_item(*name, *name)
                .directory(*directory)
                .status(*status)
                .matches(found(name, query), found(directory, query))
        });
    file_list(id, "Changes", FILTERABLE.len(), (120, 48))
        .actions(icon_button("menu", Icon::Kebab).tooltip("Changes menu"))
        .children(rows)
}

/// As wide as the docs site's stories are, 340px.
fn sized(list: impl IntoElement) -> AnyElement {
    div().w(dp(340.)).child(list).into_any_element()
}

enum Node {
    Dir(&'static str, Vec<Node>),
    /// A file, what happened to it and the lines it adds and removes.
    File(&'static str, FileStatus, (u32, u32)),
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
                        Node::File("Button.tsx", Modification, (18, 9)),
                        Node::File("Tooltip.tsx", Addition, (64, 0)),
                    ],
                ),
                Node::Dir(
                    "styles",
                    vec![
                        Node::File("theme.css", Modification, (20, 12)),
                        Node::File("reset.css", Deletion, (0, 16)),
                        Node::File("tokens.css", Addition, (14, 0)),
                    ],
                ),
                Node::File("index.ts", Modification, (92, 0)),
            ],
        ),
        Node::File("package.json", Modification, (2, 2)),
    ]
}

/// The lines the files under `node` add and remove.
fn line_counts(node: &Node) -> (u32, u32) {
    match node {
        Node::File(.., counts) => *counts,
        Node::Dir(_, children) => (children.iter().map(line_counts))
            .fold((0, 0), |(a, r), (added, removed)| (a + added, r + removed)),
    }
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
            Node::Dir(name, _) | Node::File(name, ..) => name,
        }
    }
}

type Handler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// The tree as it shows with `folded` folded and `checked` checked: `on_fold` and `on_check` give
/// a directory's chevron and a row's checkbox what to do, by path, and the files a directory's
/// checkbox stands for.
fn tree(
    id: &'static str,
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
                Node::File(_, status, _) => row.status(*status),
                Node::Dir(..) => {
                    let (added, removed) = line_counts(node);
                    let row = (row.folded(folded.contains(path.as_str())))
                        .count(files.len())
                        .line_counts(added, removed);
                    match on_fold(path.clone()) {
                        Some(handler) => row.on_toggle_folded(handler),
                        None => row.on_toggle_folded(|_, _, _| {}),
                    }
                }
            }
            .into_any_element()
        },
    );
    file_list(id, "Changes", 6, (214, 37))
        .children(rows)
        .into_any_element()
}

/// FileList's stories, then the tree to fold and check and the filter to type in.
pub struct FileListStory {
    folded: HashSet<String>,
    checked: HashSet<String>,
    /// The live filter's field, made on the first draw, and whether it's open.
    query: Option<Entity<TextInput>>,
    filtering: bool,
}

impl Default for FileListStory {
    fn default() -> Self {
        Self {
            folded: HashSet::from(["src/styles".to_owned()]),
            checked: HashSet::from(["src/components/Button.tsx".to_owned()]),
            query: None,
            filtering: false,
        }
    }
}

impl FileListStory {
    fn query(&mut self, cx: &mut Context<Self>) -> Entity<TextInput> {
        if let Some(query) = &self.query {
            return query.clone();
        }
        let query = cx.new(|cx| TextInput::new("Filter files", false, cx));
        cx.subscribe(&query, |this, _, event: &InputEvent, cx| {
            if let InputEvent::Cancel = event {
                this.filtering = false;
            }
            cx.notify();
        })
        .detach();
        self.query = Some(query.clone());
        query
    }
}

impl Render for FileListStory {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        let folded: HashSet<&str> = self.folded.iter().map(String::as_str).collect();
        let checked: HashSet<&str> = self.checked.iter().map(String::as_str).collect();
        let live_tree = tree(
            "live-tree",
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
        let query = self.query(cx);
        let typed = query.read(cx).text().to_owned();
        let focused = query.focus_handle(cx).is_focused(window);
        let live_filter = match self.filtering {
            true => {
                let found = (FILTERABLE.iter())
                    .filter(|(name, directory, _)| format!("{directory}/{name}").contains(&typed))
                    .count();
                filtered("live-filter", Some(&typed))
                    .filter(
                        query,
                        focused,
                        cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.filtering = false;
                            cx.notify();
                        }),
                    )
                    .when(!typed.is_empty(), |list| list.found(found))
            }
            false => filtered("live-filter", None).on_open_filter(cx.listener(
                move |this, _: &ClickEvent, window, cx| {
                    this.filtering = true;
                    query.update(cx, |query, cx| query.set_text("", cx));
                    window.focus(&query.focus_handle(cx), cx);
                    cx.notify();
                },
            )),
        };
        page((EXAMPLES.into_iter().map(|e| example(e, &theme))).chain([
            section("Tree, to fold and check", &theme, [sized(live_tree)]),
            section("Filter, to type in", &theme, [sized(live_filter)]),
        ]))
    }
}

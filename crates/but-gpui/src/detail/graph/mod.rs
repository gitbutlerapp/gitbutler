//! The Graph page: the project's commits as a graph, from the oldest up, each lane of the
//! workspace in a colour of its own beside the target's line, and the history it merged to
//! the left. It pans and zooms with a trackpad's gestures or a mouse: far away a commit is a
//! dot, nearer it's a pill with its title, and hovered it lights up and shows its whole
//! message, which a click keeps open, lighting up all it came from.
//!
//! It reads the graph again whenever the workspace reloads with something new, as when an
//! agent commits or a fetch brings commits in, and moves each commit it had to where it goes
//! now, while the new ones grow out of their parents.

mod layout;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bstr::BStr;
use gpui::{
    AnyElement, App, Bounds, BoxShadow, ClickEvent, ContentMask, Context, Corners, CursorStyle,
    DispatchPhase, Entity, Font, FontWeight, Hitbox, HitboxBehavior, Hsla, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PathBuilder, PinchEvent, Pixels, Point,
    ScrollDelta, ScrollWheelEvent, SharedString, Size, Subscription, Task, TextAlign, TextRun,
    TransformationMatrix, WeakEntity, Window, canvas, div, fill, font, point, prelude::*, quad,
    size,
};

use crate::backend::{
    self, Agent, Branch, CommitId, CommitState, FullName, PushState, ReviewState, Workspace,
    short_ref,
};
use crate::state::{ProjectId, Selection, Store, StoreEvent};
use crate::theme::Theme;
use crate::ui::person::byline_avatar;
use crate::ui::{
    ButtonStyle, Icon, ago, button, commit_title, dp, empty_state, fixed, h_flex, icon_button,
    popover, small_icon, v_flex,
};

use layout::{Input, Line, Route, Side, Track};

/// How far apart rows are, in pixels at 100%.
const ROW: f32 = 72.;
/// A commit up close: a pill with its title, taller nearer still, with who wrote it and when.
const PILL: Size<f32> = Size {
    width: 224.,
    height: 30.,
};
const CARD_HEIGHT: f32 = 44.;
/// A commit far away.
const DOT: f32 = 18.;
/// How wide things are across, for packing lines side by side: a merge is a circle the
/// pill's height, and a line passing a row it has nothing on takes room for itself.
const WIDTHS: layout::Widths = layout::Widths {
    pill: PILL.width,
    merge: PILL.height,
    line: 12.,
    gap: 24.,
};
/// The title's size in a pill at 100%.
const TITLE_SIZE: f32 = 12.;
/// What an empty branch's card says.
const NO_COMMITS: &str = "No commits yet";
/// How many commits of history it reads below and beside the workspace's at first, and more
/// each time the oldest of them come into view.
const HISTORY: usize = 400;
/// How far it zooms out and in.
const ZOOM: (f32, f32) = (0.02, 3.);
/// How long commits take to move where a reload puts them, the camera to move where a button
/// sends it, and a new commit's ripple to fade.
const MOVE: Duration = Duration::from_millis(520);
const GLIDE: Duration = Duration::from_millis(260);
const RIPPLE: Duration = Duration::from_millis(1600);
/// How long the spinner takes to go round while it reads further back.
const SPIN: Duration = Duration::from_millis(900);

/// A commit as the page draws it.
struct Node {
    id: CommitId,
    /// What it's known by across reloads: in the workspace its change ID, so an amend moves it
    /// rather than drawing another, else its id.
    key: SharedString,
    title: SharedString,
    body: SharedString,
    author: SharedString,
    email: SharedString,
    agent: Option<Agent>,
    time: i64,
    kind: Kind,
    labels: Vec<Label>,
    /// Its centre, in pixels at 100% from the bottom of the middle column: up is negative.
    at: Point<f32>,
    /// Its first parent in the graph, which it grows out of when it's new.
    parent: Option<usize>,
    /// It has parents the graph stops above.
    cut: bool,
    shape: Shape,
    /// The key of the branch it's the newest commit of, which the branch has while it has no
    /// commits, so its first commit takes the place of its tags, and they take its last's.
    slot: Option<SharedString>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    Commit,
    /// A circle, as a merge's title only says what it merged.
    Merge,
    /// A branch with no commits of its own: only its tags, where its first commit goes, with
    /// the edge from the commit its ref points at, which is `id`, running up into them.
    Empty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// On the workspace's lane `n`, as it's pushed and whether it's conflicted.
    Lane(usize, CommitState, bool),
    /// Only the remote of a branch on lane `n` has it.
    Remote(usize),
    /// New on the target since the workspace was updated from it.
    Incoming,
    /// The target's history, and what it merged.
    History,
}

#[derive(Debug, Clone)]
struct Label {
    text: SharedString,
    look: Look,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Look {
    /// A branch, its remote on lane `n`, or the linked worktree lane `n` is checked out in.
    Branch(Tint),
    Remote(usize),
    Worktree(usize),
    /// A branch checked out in a linked worktree whose name it says already, as an agent's
    /// often does, so one tag stands for both.
    CheckedOut(Tint),
    /// The target, like `origin/master`, and the local branch tracking it, like `master`,
    /// which looks the same, as it names the target too.
    Target,
    Local,
}

impl Look {
    /// Whether its tag shows the worktree glyph.
    fn in_worktree(self) -> bool {
        match self {
            Look::Worktree(_) | Look::CheckedOut(_) => true,
            Look::Branch(_) | Look::Remote(_) | Look::Target | Look::Local => false,
        }
    }
}

/// A graph laid out, built off the UI thread.
struct Scene {
    nodes: Vec<Node>,
    edges: Vec<layout::Edge>,
    /// By key, to carry each commit across reloads.
    keys: HashMap<SharedString, usize>,
    /// What each commit's edges reach down to, for lighting up what it came from.
    parents: Vec<Vec<usize>>,
    /// The commits' centres' extent, in the units of `Node::at`.
    extent: Bounds<f32>,
    /// How many commits of history it read, and whether there's more below them.
    history: usize,
    more: bool,
}

/// What the workspace says of the commits it has, which the graph colours and names them by.
#[derive(Default)]
struct Marks {
    /// Its commits, lane by lane, then the target's new ones.
    seeds: Vec<CommitId>,
    known: HashMap<CommitId, (Kind, SharedString)>,
    /// The branches' newest commits, with the key each branch has while it has none.
    slots: HashMap<CommitId, SharedString>,
    labels: HashMap<CommitId, Vec<Label>>,
    /// The branches with no commits, drawn after the commits, in [`Tip::Empty`]'s order.
    empties: Vec<Empty>,
    /// The lanes' tops, and their remotes' newest commits, laid out beside the target in order.
    tips: Vec<Tip>,
    target: Option<SharedString>,
    /// All of it hashed, which a reload that changes nothing has too.
    hash: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Tip {
    Commit(CommitId),
    /// The branch with no commits at this index of [`Marks::empties`].
    Empty(usize),
}

/// A branch with no commits, stacked on the commit its ref points at, or on the empty branch
/// below it in its lane.
struct Empty {
    name: FullName,
    lane: usize,
    labels: Vec<Label>,
    /// What's right above it in its lane, which stacks on it rather than on what it points at.
    under: Option<Tip>,
}

/// Where the graph is looked at from: a commit at `at` shows at `at * zoom * scale + offset`
/// pixels from the canvas's corner.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Camera {
    offset: Point<f32>,
    zoom: f32,
}

impl Camera {
    /// What it shows at `pixel` of the canvas, in the layout's units, at `scale`.
    fn at(self, pixel: Point<f32>, scale: f32) -> Point<f32> {
        let k = self.zoom * scale;
        point((pixel.x - self.offset.x) / k, (pixel.y - self.offset.y) / k)
    }

    /// What it shows moved `by` pixels.
    fn panned(self, by: Point<f32>) -> Camera {
        Camera {
            offset: point(self.offset.x + by.x, self.offset.y + by.y),
            ..self
        }
    }
}

struct Drag {
    from: Point<Pixels>,
    offset: Point<f32>,
    moved: bool,
    /// The commit it started on, which a press that doesn't move pins.
    on: Option<usize>,
}

pub struct GraphView {
    store: Entity<Store>,
    project: Option<ProjectId>,
    scene: Option<Arc<Scene>>,
    failed: Option<SharedString>,
    loading: bool,
    queued: bool,
    /// The hash of what the last load asked for, and how much history.
    asked: Option<(u64, usize)>,
    /// How many commits of history it reads, more once the oldest are in view.
    history: usize,
    /// Where the camera was when reading further back last landed. With the oldest commits
    /// still in view it reads on only once the camera moves, so zoomed far out it doesn't
    /// read on and on by itself.
    deepened: Option<Camera>,
    /// When it started reading further back, which a spinner under the oldest commits turns
    /// from until what it read lands.
    deepening: Option<Instant>,
    camera: Camera,
    /// Whether the camera has been put where the graph opens, which waits for the canvas's
    /// size.
    placed: bool,
    /// The camera on its way from one place to another, since when.
    glide: Option<(Instant, Camera, Camera)>,
    /// Where each commit was drawn when the last load landed, and when it did.
    moved: Option<(Instant, HashMap<SharedString, Point<f32>>)>,
    /// The commits that load brought, new or rewritten, and when it landed.
    ripples: (Vec<usize>, Instant),
    /// What that load brought in place of something of the same branch: a branch's first
    /// commit, growing where its tags were, and an emptied branch's tags, where its last commit
    /// was. Their tags stay, moving over or into its place.
    swaps: Rc<HashSet<usize>>,
    hovered: Option<usize>,
    hovered_at: Instant,
    /// The edge under the pointer, by its place in the scene's, when no commit is.
    hovered_edge: Option<usize>,
    pinned: Option<SharedString>,
    /// The pinned commit and its ancestors.
    lineage: Rc<HashSet<usize>>,
    drag: Option<Drag>,
    /// The canvas's bounds as last drawn, in the window.
    bounds: Rc<Cell<Bounds<Pixels>>>,
    /// Where each commit was drawn last, for finding the one under the pointer.
    drawn: Rc<Vec<Point<f32>>>,
    /// Titles cut to fit their pill, made when first drawn.
    titles: Rc<RefCell<HashMap<usize, SharedString>>>,
    /// Where each branch with no commits had its tags drawn last, in the canvas's pixels:
    /// with no pill, they're what's pointed at.
    tags: Rc<RefCell<HashMap<usize, Bounds<f32>>>>,
    /// The interface scale as last drawn.
    scale: f32,
    _load: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl GraphView {
    pub fn new(store: Entity<Store>, cx: &mut Context<Self>) -> Self {
        let subscriptions = Vec::from([cx.subscribe(&store, |this, _, event, cx| match event {
            StoreEvent::SelectionChanged | StoreEvent::Loaded(_) => this.follow(cx),
            // Picked again in the navigator, it goes back to how it opens.
            StoreEvent::Reopened => {
                if this.selected(cx).is_some()
                    && let Some(scene) = this.scene.clone()
                {
                    let to = this.opening(&scene);
                    this.glide_to(to, cx);
                }
            }
            StoreEvent::Renamed(..)
            | StoreEvent::MeshRead(_)
            | StoreEvent::Pending(_)
            | StoreEvent::AuthorMissing(_) => {}
        })]);
        GraphView {
            store,
            project: None,
            scene: None,
            failed: None,
            loading: false,
            queued: false,
            asked: None,
            history: HISTORY,
            deepened: None,
            deepening: None,
            camera: Camera {
                offset: point(0., 0.),
                zoom: 1.,
            },
            placed: false,
            glide: None,
            moved: None,
            ripples: (Vec::new(), Instant::now()),
            swaps: Rc::default(),
            hovered: None,
            hovered_at: Instant::now(),
            hovered_edge: None,
            pinned: None,
            lineage: Rc::default(),
            drag: None,
            bounds: Rc::default(),
            drawn: Rc::default(),
            titles: Rc::default(),
            tags: Rc::default(),
            scale: 1.,
            _load: Task::ready(()),
            _subscriptions: subscriptions,
        }
    }

    /// The project whose graph is selected.
    fn selected(&self, cx: &App) -> Option<ProjectId> {
        let Some(Selection::Graph(project)) = &self.store.read(cx).selection else {
            return None;
        };
        Some(project.clone())
    }

    /// Show the graph selected, reading it again when the workspace has something new.
    fn follow(&mut self, cx: &mut Context<Self>) {
        let Some(project) = self.selected(cx) else {
            return;
        };
        if self.project.as_ref() != Some(&project) {
            self.project = Some(project);
            self.scene = None;
            self.failed = None;
            self.asked = None;
            self.history = HISTORY;
            self.deepened = None;
            self.deepening = None;
            self.placed = false;
            self.glide = None;
            self.moved = None;
            self.ripples.0.clear();
            self.unhover();
            self.pinned = None;
            self.lineage = Rc::default();
            cx.notify();
        }
        self.load(cx);
    }

    fn load(&mut self, cx: &mut Context<Self>) {
        let Some(project) = self.project.clone() else {
            return;
        };
        let store = self.store.read(cx);
        let (Some(path), Some(workspace)) = (store.path(&project), store.workspace(&project))
        else {
            return;
        };
        let marks = marks(&project, store, workspace);
        let asking = (marks.hash, self.history);
        if self.asked == Some(asking) && self.failed.is_none() {
            return;
        }
        // One load at a time, as `backend::run` can't be cancelled; the newest goes next.
        if self.loading {
            self.queued = true;
            return;
        }
        self.loading = true;
        self.asked = Some(asking);
        let history = self.history;
        self._load = cx.spawn(async move |this, cx| {
            let result = backend::run(move || {
                let refs: Vec<FullName> = (marks.empties.iter())
                    .map(|empty| empty.name.clone())
                    .collect();
                let graph = backend::graph::graph(&path, &marks.seeds, &refs, history)?;
                Ok(build(graph, marks, history))
            })
            .await;
            this.update(cx, |this, cx| {
                this.loading = false;
                if this.project.as_ref() == Some(&project) {
                    match result {
                        Ok(scene) => this.show(Arc::new(scene)),
                        Err(err) => this.failed = Some(format!("{err:#}").into()),
                    }
                }
                if std::mem::take(&mut this.queued) {
                    this.load(cx);
                }
                cx.notify();
            })
            .ok();
        });
    }

    /// Put `scene` on screen, each commit moving from where it was to where it goes now.
    fn show(&mut self, scene: Arc<Scene>) {
        let now = Instant::now();
        self.failed = None;
        if let Some(old) = self.scene.clone() {
            let positions = self.drawn.clone();
            let drawn = |ix: usize| {
                (positions.get(ix).copied()).or_else(|| old.nodes.get(ix).map(|node| node.at))
            };
            let mut from: HashMap<SharedString, Point<f32>> = (0..old.nodes.len())
                .filter_map(|ix| Some((old.nodes.get(ix)?.key.clone(), drawn(ix)?)))
                .collect();
            // Reading further back lays out all above the history it adds higher up, so the
            // view moves with what's in its middle, and all moves only as it does from there.
            let deeper = old.history != scene.history;
            if let Some(by) = deeper.then(|| self.drift(&old, &scene)).flatten() {
                for at in from.values_mut() {
                    *at = point(at.x + by.x, at.y + by.y);
                }
                let k = self.k();
                self.pan(point(-by.x * k, -by.y * k));
            }
            if deeper {
                self.deepened = Some(self.camera);
                self.deepening = None;
            }
            let mut swaps = HashSet::new();
            for (ix, node) in scene.nodes.iter().enumerate() {
                if old.keys.contains_key(&node.key) {
                    continue;
                }
                let stood = match node.shape {
                    Shape::Commit | Shape::Merge => (node.slot.as_ref())
                        .and_then(|slot| old.keys.get(slot).copied())
                        .filter(|was| {
                            (old.nodes.get(*was)).is_some_and(|was| was.shape == Shape::Empty)
                        }),
                    Shape::Empty => {
                        (old.nodes.iter()).position(|was| was.slot.as_ref() == Some(&node.key))
                    }
                };
                if let Some(at) = stood.and_then(drawn) {
                    from.insert(node.key.clone(), at);
                    swaps.insert(ix);
                }
            }
            // Older commits read aren't news.
            let ripples = (scene.nodes.iter().enumerate())
                .filter(|_| !deeper)
                .filter(|(_, node)| {
                    (old.keys.get(&node.key).and_then(|was| old.nodes.get(*was)))
                        .is_none_or(|was| was.id != node.id)
                })
                .map(|(ix, _)| ix)
                .collect();
            self.ripples = (ripples, now);
            self.swaps = Rc::new(swaps);
            self.moved = Some((now, from));
        }
        // A pan goes on through a reload, but what it was pressed on may have gone.
        self.unhover();
        if let Some(drag) = &mut self.drag {
            drag.on = None;
        }
        // Until it's drawn, where it's going stands for where it is, so a load landing before
        // then moves from the right places.
        self.drawn = Rc::new(scene.nodes.iter().map(|node| node.at).collect());
        self.titles = Rc::default();
        self.lineage = Rc::new(
            (self.pinned.as_ref())
                .and_then(|key| scene.keys.get(key))
                .map(|pinned| lineage(&scene, *pinned))
                .unwrap_or_default(),
        );
        if self.lineage.is_empty() {
            self.pinned = None;
        }
        self.scene = Some(scene);
    }

    /// The canvas grew by `grown` pixels: what was in its middle stays there.
    fn resized(&mut self, grown: Point<f32>) {
        self.pan(point(grown.x / 2., grown.y / 2.));
    }

    /// Move what's shown `by` pixels, and where a glide or a pan is taking it.
    fn pan(&mut self, by: Point<f32>) {
        self.camera = self.camera.panned(by);
        if let Some((_, from, to)) = &mut self.glide {
            *from = from.panned(by);
            *to = to.panned(by);
        }
        if let Some(drag) = &mut self.drag {
            drag.offset = point(drag.offset.x + by.x, drag.offset.y + by.y);
        }
    }

    /// How far the commit drawn nearest the middle of the view moves from `old` to `new`, in
    /// the layout's units.
    fn drift(&self, old: &Scene, new: &Scene) -> Option<Point<f32>> {
        let view = self.bounds.get().size;
        let middle = point(f32::from(view.width) / 2., f32::from(view.height) / 2.);
        let middle = self.camera.at(middle, self.scale);
        let away = |at: &Point<f32>| (at.x - middle.x).powi(2) + (at.y - middle.y).powi(2);
        let (was, now) = (self.drawn.iter().zip(&old.nodes))
            .filter_map(|(at, node)| Some((at, new.nodes.get(*new.keys.get(&node.key)?)?.at)))
            .min_by(|(a, _), (b, _)| away(a).total_cmp(&away(b)))?;
        Some(point(now.x - was.x, now.y - was.y))
    }

    /// Nothing under the pointer, as when what was there moves away.
    fn unhover(&mut self) {
        self.hovered = None;
        self.hovered_edge = None;
    }

    /// Pixels a unit of the layout takes now.
    fn k(&self) -> f32 {
        self.camera.zoom * self.scale
    }

    /// Where the graph opens: the newest commits at the top, at 100% when they fit across.
    fn opening(&self, scene: &Scene) -> Camera {
        let view = self.bounds.get().size;
        let (width, height) = (f32::from(view.width), f32::from(view.height));
        let top = scene.extent.origin.y;
        let newest = (scene.nodes.iter()).filter(|node| node.at.y <= top + 10. * ROW);
        let (left, right) = newest.fold((f32::MAX, f32::MIN), |(left, right), node| {
            (left.min(node.at.x), right.max(node.at.x))
        });
        let across = right - left + PILL.width + 2. * WIDTHS.gap;
        let zoom = (width / (across * self.scale)).clamp(0.45, 1.);
        let k = zoom * self.scale;
        Camera {
            offset: point(
                width / 2. - (left + right) / 2. * k,
                (height * 0.12).min(96.) - top * k,
            ),
            zoom,
        }
    }

    /// The workspace's commits in view, with what they start from, or where the graph opens
    /// when it has none.
    fn on_workspace(&self, scene: &Scene) -> Camera {
        let lanes = (scene.nodes.iter())
            .filter(|node| matches!(node.kind, Kind::Lane(..) | Kind::Remote(_)));
        let starts = lanes
            .clone()
            .filter_map(|node| scene.nodes.get(node.parent?));
        match extent_of(lanes.chain(starts).map(|node| node.at)) {
            Some(extent) => self.framing(extent),
            None => self.opening(scene),
        }
    }

    /// The target's newest commit in the middle of the view, at 100% or nearer.
    fn on_target(&self, scene: &Scene) -> Option<Camera> {
        let tip = (scene.nodes.iter())
            .find(|node| (node.labels.iter()).any(|label| label.look == Look::Target))?;
        Some(self.centred(tip.at, self.camera.zoom.max(1.)))
    }

    /// `extent`, in the layout's units, in view, at 100% at most.
    fn framing(&self, extent: Bounds<f32>) -> Camera {
        let view = self.bounds.get().size;
        let (width, height) = (f32::from(view.width), f32::from(view.height));
        let (across, down) = (
            extent.size.width + PILL.width + 96.,
            extent.size.height + PILL.height + 96.,
        );
        let zoom = (width / (across * self.scale))
            .min(height / (down * self.scale))
            .clamp(ZOOM.0, 1.);
        self.centred(extent.center(), zoom)
    }

    /// `at`, in the layout's units, in the middle of the view at `zoom`.
    fn centred(&self, at: Point<f32>, zoom: f32) -> Camera {
        let view = self.bounds.get().size;
        let k = zoom * self.scale;
        Camera {
            offset: point(
                f32::from(view.width) / 2. - at.x * k,
                f32::from(view.height) / 2. - at.y * k,
            ),
            zoom,
        }
    }

    fn glide_to(&mut self, to: Camera, cx: &mut Context<Self>) {
        self.glide = Some((Instant::now(), self.camera, to));
        // What's under the pointer moves away; the next move finds what's there then.
        self.unhover();
        cx.notify();
    }

    /// Zoom by `factor` keeping what's at `at`, in the canvas's pixels, where it is.
    fn zoom_at(&mut self, at: Point<f32>, factor: f32) {
        self.camera = zoomed(self.camera, at, factor);
        self.glide = None;
    }

    /// Zoom by `factor` about the middle of the view, gliding there.
    fn zoom_by(&mut self, factor: f32, cx: &mut Context<Self>) {
        let size = self.bounds.get().size;
        let middle = point(f32::from(size.width) / 2., f32::from(size.height) / 2.);
        let from = self.glide.map_or(self.camera, |(_, _, to)| to);
        self.glide_to(zoomed(from, middle, factor), cx);
    }

    /// `position`, in the window, from the canvas's corner.
    fn local(&self, position: Point<Pixels>) -> Point<f32> {
        let origin = self.bounds.get().origin;
        point(
            f32::from(position.x - origin.x),
            f32::from(position.y - origin.y),
        )
    }

    /// The commit drawn under `at`, in the canvas's pixels.
    fn hit(&self, at: Point<f32>) -> Option<usize> {
        let k = self.k();
        let sizes = Sizes::at(k);
        let offset = self.camera.offset;
        let nodes = self.scene.as_ref()?.nodes.iter();
        let tags = self.tags.borrow();
        (self.drawn.iter().zip(nodes).enumerate().rev()).find_map(|(ix, (centre, node))| {
            let (x, y, reach) = match tags.get(&ix) {
                Some(row) => (middle(row).x, middle(row).y, row.size),
                None => (
                    centre.x * k + offset.x,
                    centre.y * k + offset.y,
                    sizes.of(node),
                ),
            };
            ((at.x - x).abs() <= reach.width / 2. + 3.
                && (at.y - y).abs() <= reach.height / 2. + 3.)
                .then_some(ix)
        })
    }

    /// The edge drawn nearest `at`, in the canvas's pixels, if it's close enough to point at.
    fn edge_at(&self, at: Point<f32>) -> Option<usize> {
        let scene = self.scene.as_ref()?;
        let (k, reach) = (self.k(), 6. * self.scale);
        let sizes = Sizes::at(k);
        let spacing = Spacing::at(k, sizes);
        let near = (scene.edges.iter().enumerate()).filter_map(|(ix, edge)| {
            let ends = edge_ends(
                edge,
                &scene.nodes,
                &self.drawn,
                sizes,
                k,
                self.camera.offset,
            )?;
            let Ends { start, end, .. } = ends;
            // An edge across rows goes up beside its child.
            let aside = edge.aside.abs() * k;
            let outside = at.x < start.x.min(end.x) - reach - aside
                || at.x > start.x.max(end.x) + reach + aside
                || at.y < end.y - reach
                || at.y > start.y + reach;
            if outside {
                return None;
            }
            let away = distance(at, start, stretches(edge, ends, spacing));
            (away <= reach).then_some((ix, away))
        });
        near.min_by(|a, b| a.1.total_cmp(&b.1)).map(|(ix, _)| ix)
    }

    fn pin(&mut self, node: Option<usize>, cx: &mut Context<Self>) {
        let Some(scene) = self.scene.clone() else {
            return;
        };
        let key = (node.and_then(|ix| scene.nodes.get(ix))).map(|node| node.key.clone());
        let unpin = key.is_none() || key == self.pinned;
        self.pinned = if unpin { None } else { key };
        self.lineage = Rc::new(match (unpin, node) {
            (false, Some(ix)) => lineage(&scene, ix),
            (true, _) | (false, None) => HashSet::new(),
        });
        cx.notify();
    }

    fn pressed(&mut self, event: &MouseDownEvent, cx: &mut Context<Self>) {
        let at = self.local(event.position);
        let on = self.hit(at);
        // Into a commit.
        if let Some(centre) =
            (on.and_then(|ix| self.drawn.get(ix).copied())).filter(|_| event.click_count == 2)
        {
            let to = self.centred(centre, self.camera.zoom.max(1.4));
            self.drag = None;
            self.glide_to(to, cx);
            return;
        }
        self.glide = None;
        self.drag = Some(Drag {
            from: event.position,
            offset: self.camera.offset,
            moved: false,
            on,
        });
    }

    fn moved_to(&mut self, event: &MouseMoveEvent, over: bool, cx: &mut Context<Self>) {
        if let Some(drag) = &mut self.drag {
            if event.pressed_button != Some(MouseButton::Left) {
                self.drag = None;
            } else {
                let by = event.position - drag.from;
                let (x, y) = (f32::from(by.x), f32::from(by.y));
                if drag.moved || x.hypot(y) > 3. {
                    drag.moved = true;
                    self.camera.offset = point(drag.offset.x + x, drag.offset.y + y);
                    self.unhover();
                    cx.notify();
                    return;
                }
            }
        }
        let at = self.local(event.position);
        let hovered = over.then(|| self.hit(at)).flatten();
        let edge = (over && hovered.is_none())
            .then(|| self.edge_at(at))
            .flatten();
        if hovered != self.hovered {
            self.hovered = hovered;
            self.hovered_at = Instant::now();
            cx.notify();
        }
        if edge != self.hovered_edge {
            self.hovered_edge = edge;
            cx.notify();
        }
    }

    fn released(&mut self, cx: &mut Context<Self>) {
        let Some(drag) = self.drag.take() else {
            return;
        };
        if !drag.moved {
            self.pin(drag.on, cx);
        }
        cx.notify();
    }

    /// A trackpad's two fingers pan, as does a mouse's wheel with shift; the wheel zooms, as do
    /// two fingers with ⌘ or ctrl.
    fn scrolled(&mut self, event: &ScrollWheelEvent, cx: &mut Context<Self>) {
        let at = self.local(event.position);
        let zooms = event.modifiers.platform || event.modifiers.control;
        match event.delta {
            ScrollDelta::Pixels(by) if !zooms => {
                let (x, y) = (f32::from(by.x), f32::from(by.y));
                self.camera.offset = point(self.camera.offset.x + x, self.camera.offset.y + y);
                self.glide = None;
            }
            ScrollDelta::Pixels(by) => self.zoom_at(at, (f32::from(by.y) * 0.01).exp()),
            ScrollDelta::Lines(by) if event.modifiers.shift => {
                let x = (by.y + by.x) * 48.;
                self.camera.offset = point(self.camera.offset.x + x, self.camera.offset.y);
                self.glide = None;
            }
            ScrollDelta::Lines(by) => self.zoom_at(at, 1.15f32.powf(by.y)),
        }
        self.unhover();
        cx.notify();
    }

    fn pinched(&mut self, event: &PinchEvent, cx: &mut Context<Self>) {
        self.zoom_at(self.local(event.position), 1. + event.delta);
        self.unhover();
        cx.notify();
    }

    /// Where each commit is drawn at `now`, how much of it shows as it grows in, and whether
    /// anything is still moving.
    fn frame(&mut self, scene: &Scene, now: Instant) -> (Vec<Point<f32>>, Vec<f32>, bool) {
        let mut moving = false;
        if let Some((start, from, to)) = self.glide {
            let t = ease(now.duration_since(start).as_secs_f32() / GLIDE.as_secs_f32());
            let view = self.bounds.get().size;
            let middle = point(f32::from(view.width) / 2., f32::from(view.height) / 2.);
            self.camera = glided(from, to, t, middle, self.scale);
            if t >= 1. {
                self.glide = None;
            } else {
                moving = true;
            }
        }
        let t = (self.moved.as_ref())
            .map(|(start, _)| ease(now.duration_since(*start).as_secs_f32() / MOVE.as_secs_f32()))
            .unwrap_or(1.);
        let from = self.moved.as_ref().map(|(_, from)| from);
        let mut positions = Vec::with_capacity(scene.nodes.len());
        let mut grown = Vec::with_capacity(scene.nodes.len());
        let swaps = self.swaps.clone();
        for (ix, node) in scene.nodes.iter().enumerate() {
            let was = from.and_then(|from| from.get(&node.key));
            let start = was.copied().or_else(|| {
                let parent = scene.nodes.get(node.parent?)?;
                from.and_then(|from| from.get(&parent.key)).copied()
            });
            let start = start.filter(|_| t < 1.).unwrap_or(node.at);
            positions.push(point(
                lerp(start.x, node.at.x, t),
                lerp(start.y, node.at.y, t),
            ));
            // New, it grows in where it starts: out of its parent, or in another's place.
            grown.push(match (from, was) {
                (Some(_), None) => t,
                (Some(_), Some(_)) if swaps.contains(&ix) => t,
                (Some(_), Some(_)) | (None, _) => 1.,
            });
        }
        if t >= 1. {
            self.moved = None;
        } else {
            moving = true;
        }
        let rippling = !self.ripples.0.is_empty() && self.ripple_age(now) < 1.;
        if !rippling {
            self.ripples.0.clear();
        }
        (positions, grown, moving || rippling)
    }

    /// How far the last load's ripples are through, from 0 to 1 and past.
    fn ripple_age(&self, now: Instant) -> f32 {
        now.duration_since(self.ripples.1).as_secs_f32() / RIPPLE.as_secs_f32()
    }
}

/// Part `t` of the way from `from` to `to`: what's at `middle` on screen goes straight from
/// one to the other, zooming evenly on the way, rather than the offset going straight and
/// what's looked at swinging round as the zoom changes.
fn glided(from: Camera, to: Camera, t: f32, middle: Point<f32>, scale: f32) -> Camera {
    let zoom = from.zoom * (to.zoom / from.zoom).powf(t);
    let (a, b) = (from.at(middle, scale), to.at(middle, scale));
    let at = point(lerp(a.x, b.x, t), lerp(a.y, b.y, t));
    let k = zoom * scale;
    Camera {
        offset: point(middle.x - at.x * k, middle.y - at.y * k),
        zoom,
    }
}

/// `camera` zoomed by `factor`, keeping what's at `at` there.
fn zoomed(camera: Camera, at: Point<f32>, factor: f32) -> Camera {
    let zoom = (camera.zoom * factor).clamp(ZOOM.0, ZOOM.1);
    let ratio = zoom / camera.zoom;
    Camera {
        offset: point(
            at.x - (at.x - camera.offset.x) * ratio,
            at.y - (at.y - camera.offset.y) * ratio,
        ),
        zoom,
    }
}

fn lerp(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

/// Out of `t`, clamped to 0 to 1, fast and then slowing, as things come to rest.
fn ease(t: f32) -> f32 {
    1. - (1. - t.clamp(0., 1.)).powi(3)
}

/// From 0 below `from` to 1 above `to`, smoothly.
fn smooth(x: f32, from: f32, to: f32) -> f32 {
    let t = ((x - from) / (to - from)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}

/// How big commits draw at some zoom: from a dot far away to a pill with its title, and up
/// close a second line.
#[derive(Debug, Clone, Copy)]
struct Sizes {
    pill: Size<f32>,
    /// A merge's circle, the pill's height, as its title only says what it merged.
    merge: f32,
    /// How far a dot has opened into a pill, and how much of the second line shows, from 0
    /// to 1.
    opened: f32,
    card: f32,
}

impl Sizes {
    /// At `k` pixels a unit.
    fn at(k: f32) -> Sizes {
        let (opened, card) = (smooth(k, 0.44, 0.58), smooth(k, 1.15, 1.45));
        let dot = (DOT * k).max(5f32.min(0.8 * ROW * k));
        let height = lerp(PILL.height, CARD_HEIGHT, card) * k;
        Sizes {
            pill: size(lerp(dot, PILL.width * k, opened), lerp(dot, height, opened)),
            merge: lerp(dot, PILL.height * k, opened),
            opened,
            card,
        }
    }

    fn of(self, node: &Node) -> Size<f32> {
        match node.shape {
            Shape::Commit => self.pill,
            Shape::Merge => size(self.merge, self.merge),
            // Its edges run on into its tags, which `paint_labels` sizes.
            Shape::Empty => size(0., 0.),
        }
    }
}

/// The commit `pinned` and all it came from.
fn lineage(scene: &Scene, pinned: usize) -> HashSet<usize> {
    let mut seen = HashSet::from([pinned]);
    let mut queue = VecDeque::from([pinned]);
    while let Some(at) = queue.pop_front() {
        for reached in scene.parents.get(at).into_iter().flatten() {
            if seen.insert(*reached) {
                queue.push_back(*reached);
            }
        }
    }
    seen
}

/// A branch's colour: its lane's, or purple once the target has it, as the navigator marks it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tint {
    Lane(usize),
    Integrated,
}

impl Tint {
    fn color(self, theme: &Theme) -> Hsla {
        match self {
            Tint::Lane(lane) => hue(theme, lane),
            Tint::Integrated => theme.purple,
        }
    }
}

fn marks(project: &ProjectId, store: &Store, workspace: &Workspace) -> Marks {
    let mut marks = Marks::default();
    for (lane_ix, lane) in workspace.lanes.iter().enumerate() {
        let mut top = true;
        let mut above = None;
        for branch in &lane.branches {
            let labels = labels_of(
                store.name_of(project, branch),
                tint_of(project, store, lane_ix, branch),
                (lane.checkout.linked()).filter(|_| top),
                lane_ix,
            );
            let tip = match (branch.commits.first(), &branch.ref_name) {
                (Some(tip), name) => {
                    marks.labels.entry(tip.id).or_default().extend(labels);
                    if let Some(name) = name {
                        marks.slots.insert(tip.id, slot_key(name));
                    }
                    Some(Tip::Commit(tip.id))
                }
                (None, Some(name)) => {
                    let ix = marks.empties.len();
                    marks.empties.push(Empty {
                        name: name.clone(),
                        lane: lane_ix,
                        labels,
                        under: above,
                    });
                    Some(Tip::Empty(ix))
                }
                (None, None) => None,
            };
            if let Some(tip) = tip {
                if top {
                    marks.tips.push(tip);
                    top = false;
                }
                above = Some(match branch.commits.last() {
                    Some(lowest) => Tip::Commit(lowest.id),
                    None => tip,
                });
            }
            for commit in &branch.commits {
                marks.seeds.push(commit.id);
                let kind = Kind::Lane(lane_ix, commit.state, commit.conflicted);
                marks
                    .known
                    .insert(commit.id, (kind, commit.key.clone().into()));
            }
            if let Some(tip) = branch.incoming.first() {
                marks.tips.push(Tip::Commit(tip.id));
                marks.labels.entry(tip.id).or_default().push(Label {
                    text: branch
                        .remote
                        .clone()
                        .unwrap_or_else(|| "remote".into())
                        .into(),
                    look: Look::Remote(lane_ix),
                });
            }
            for commit in &branch.incoming {
                marks.seeds.push(commit.id);
                let key = match commit.key.as_str() {
                    "" => commit.id.0.to_string(),
                    key => key.to_owned(),
                };
                marks
                    .known
                    .insert(commit.id, (Kind::Remote(lane_ix), key.into()));
            }
        }
    }
    // A linked worktree whose branch has no commits of its own has no lane: it and its branch
    // go where the branch points, each in a lane's colour after the others.
    let laneless = (workspace.worktrees.iter()).filter(|worktree| {
        !(workspace.lanes.iter()).any(|lane| lane.checkout.linked() == Some(worktree.name.as_ref()))
    });
    for (lane_ix, worktree) in (workspace.lanes.len()..).zip(laneless) {
        let Some(name) = &worktree.branch else {
            continue;
        };
        let ix = marks.empties.len();
        marks.empties.push(Empty {
            name: name.clone(),
            lane: lane_ix,
            labels: labels_of(
                &short_ref(name),
                Tint::Lane(lane_ix),
                Some(worktree.name.as_ref()),
                lane_ix,
            ),
            under: None,
        });
        marks.tips.push(Tip::Empty(ix));
    }
    if let Some(target) = &workspace.target {
        marks.target = Some(target.name.clone().into());
        for commit in &target.incoming {
            marks.seeds.push(commit.id);
            let key = commit.id.0.to_string().into();
            marks.known.insert(commit.id, (Kind::Incoming, key));
        }
    }
    let mut hasher = std::hash::DefaultHasher::new();
    marks.seeds.hash(&mut hasher);
    workspace.refs.hash(&mut hasher);
    for (id, labels) in &marks.labels {
        id.hash(&mut hasher);
        for label in labels {
            label.text.hash(&mut hasher);
        }
    }
    for empty in &marks.empties {
        empty.name.hash(&mut hasher);
        empty.under.hash(&mut hasher);
        for label in &empty.labels {
            label.text.hash(&mut hasher);
        }
    }
    marks.hash = hasher.finish();
    marks
}

/// Branch `name`'s tags on lane `lane_ix`: the branch first, nearest its commit, then the
/// linked worktree it's checked out in, unless the branch's name says it already.
fn labels_of(name: &str, tint: Tint, worktree: Option<&BStr>, lane_ix: usize) -> Vec<Label> {
    let worktree = worktree.map(|worktree| worktree.to_string());
    let look = match &worktree {
        Some(worktree) if names(name, worktree) => Look::CheckedOut(tint),
        Some(_) | None => Look::Branch(tint),
    };
    let mut labels = Vec::from([Label {
        text: name.to_owned().into(),
        look,
    }]);
    if let Some(worktree) = worktree.filter(|_| look == Look::Branch(tint)) {
        labels.push(Label {
            text: worktree.into(),
            look: Look::Worktree(lane_ix),
        });
    }
    labels
}

/// The colour of `branch` on lane `lane_ix`: purple once it's landed, with a pull request or
/// without, as the navigator's glyph says.
fn tint_of(project: &ProjectId, store: &Store, lane_ix: usize, branch: &Branch) -> Tint {
    let merged = (branch.review.as_ref())
        .is_some_and(|review| store.review_state(project, review) == ReviewState::Merged);
    match branch.push == PushState::Integrated || merged {
        true => Tint::Integrated,
        false => Tint::Lane(lane_ix),
    }
}

/// Whether branch `name` says the name of `worktree` already, as `dp/fix` does `fix` and
/// `dp-fix`, which GitButler names a worktree for it.
fn names(name: &str, worktree: &str) -> bool {
    name.contains(worktree) || name.replace('/', "-").contains(worktree)
}

/// `graph`, with `history` commits of history, laid out, its commits coloured and named by
/// `marks`.
fn build(graph: backend::graph::Graph, mut marks: Marks, history: usize) -> Scene {
    let index: HashMap<CommitId, usize> = (graph.commits.iter().enumerate())
        .map(|(ix, commit)| (commit.id, ix))
        .collect();
    let mut inputs: Vec<Input> = (graph.commits.iter())
        .map(|commit| Input {
            parents: (commit.parents.iter())
                .map(|parent| index.get(parent).copied())
                .collect(),
        })
        .collect();
    // The branches with no commits whose refs are still there, after the commits, each on the
    // commit its ref points at. What's above one in its lane stacks on it instead, as a
    // stack's branches do, when it starts from that commit too.
    let mut slots = HashMap::new();
    let mut empties = Vec::new();
    for (ix, (empty, at)) in (std::mem::take(&mut marks.empties).into_iter())
        .zip(&graph.refs)
        .enumerate()
    {
        let Some(at) = *at else {
            continue;
        };
        slots.insert(ix, inputs.len());
        inputs.push(Input {
            parents: Vec::from([index.get(&at).copied()]),
        });
        empties.push((empty, at));
    }
    for (slot, (empty, at)) in (graph.commits.len()..).zip(&empties) {
        let above = match empty.under {
            Some(Tip::Empty(above)) => slots.get(&above).copied(),
            Some(Tip::Commit(above)) => (index.get(&above).copied()).filter(|above| {
                (graph.commits.get(*above)).and_then(|commit| commit.parents.first()) == Some(at)
            }),
            None => None,
        };
        let first = (above.and_then(|above| inputs.get_mut(above)))
            .and_then(|above| above.parents.first_mut());
        if let Some(first) = first {
            *first = Some(slot);
        }
    }
    let target = graph.target.and_then(|tip| index.get(&tip).copied());
    let local = (graph.local.as_ref())
        .and_then(|(tip, name)| Some((*index.get(tip)?, SharedString::from(name.clone()))));
    let lines: Vec<Line> = (target.map(|tip| Line {
        tip,
        side: Side::Middle,
    }))
    .into_iter()
    .chain((marks.tips.iter()).filter_map(|tip| {
        let tip = match tip {
            Tip::Commit(id) => index.get(id),
            Tip::Empty(ix) => slots.get(ix),
        };
        Some(Line {
            tip: *tip?,
            side: Side::Over,
        })
    }))
    .collect();
    let laid = layout::layout(&inputs, &lines, WIDTHS);
    let mut parents: Vec<Vec<usize>> = std::iter::repeat_n(Vec::new(), inputs.len()).collect();
    for edge in &laid.edges {
        if let Some(parents) = parents.get_mut(edge.child) {
            parents.push(edge.parent);
        }
    }
    // What the target's new commits merged in is new too: all they reach that the workspace's
    // commits don't, nor where the target's own new line starts from.
    let kinds: Vec<Option<Kind>> = (graph.commits.iter())
        .map(|commit| marks.known.get(&commit.id).map(|(kind, _)| *kind))
        .chain((empties.iter()).map(|(empty, _)| Some(empty_kind(empty))))
        .collect();
    let (mut had, mut new) = (Vec::new(), Vec::new());
    for (ix, (kind, input)) in kinds.iter().zip(&inputs).enumerate() {
        match kind {
            Some(Kind::Lane(..)) => had.push(ix),
            Some(Kind::Incoming) => {
                new.push(ix);
                let below = input.parents.first().copied().flatten();
                had.extend(below.filter(|parent| {
                    kinds.get(*parent).copied().flatten() != Some(Kind::Incoming)
                }));
            }
            Some(Kind::Remote(_) | Kind::History) | None => {}
        }
    }
    let had = reach(&inputs, had, &[]);
    let new = reach(&inputs, new, &had);
    let count = graph.commits.len();
    let mut nodes: Vec<Node> = (graph.commits.into_iter())
        .zip(inputs.iter().zip(&laid.places).zip(&new))
        .enumerate()
        .map(|(ix, (commit, ((input, place), new)))| {
            let (kind, key) = (marks.known.remove(&commit.id))
                .unwrap_or_else(|| (Kind::History, commit.id.0.to_string().into()));
            let kind = match kind {
                Kind::History if *new => Kind::Incoming,
                kind => kind,
            };
            let mut labels = marks.labels.remove(&commit.id).unwrap_or_default();
            if Some(ix) == target {
                labels.push(Label {
                    text: marks.target.clone().unwrap_or_else(|| "target".into()),
                    look: Look::Target,
                });
            }
            if let Some((_, name)) = local.as_ref().filter(|(tip, _)| *tip == ix) {
                labels.push(Label {
                    text: name.clone(),
                    look: Look::Local,
                });
            }
            Node {
                id: commit.id,
                key,
                title: commit_title(&commit.title),
                body: commit.body.into(),
                author: commit.author.into(),
                email: commit.email.into(),
                agent: commit.agent,
                time: commit.time,
                kind,
                labels,
                at: point(place.x, -(place.row as f32) * ROW),
                parent: input.parents.first().copied().flatten(),
                cut: input.parents.iter().any(Option::is_none),
                shape: match commit.parents.len() > 1 {
                    true => Shape::Merge,
                    false => Shape::Commit,
                },
                slot: marks.slots.remove(&commit.id),
            }
        })
        .collect();
    let rest = inputs.iter().zip(&laid.places).skip(count);
    nodes.extend(
        (empties.into_iter().zip(rest)).map(|((empty, at), (input, place))| Node {
            id: at,
            key: slot_key(&empty.name),
            title: SharedString::default(),
            body: SharedString::default(),
            author: SharedString::default(),
            email: SharedString::default(),
            agent: None,
            time: 0,
            kind: empty_kind(&empty),
            labels: empty.labels,
            at: point(place.x, -(place.row as f32) * ROW),
            parent: input.parents.first().copied().flatten(),
            cut: input.parents.iter().any(Option::is_none),
            shape: Shape::Empty,
            slot: None,
        }),
    );
    let extent = extent_of(nodes.iter().map(|node| node.at))
        .unwrap_or_else(|| Bounds::new(point(0., 0.), size(0., 0.)));
    Scene {
        keys: (nodes.iter().enumerate())
            .map(|(ix, node)| (node.key.clone(), ix))
            .collect(),
        // History it didn't read is below the commits that have parents it doesn't.
        more: nodes.iter().any(|node| node.cut),
        nodes,
        edges: laid.edges,
        parents,
        extent,
        history,
    }
}

/// The smallest bounds holding all of `points`, if there are any.
fn extent_of(points: impl Iterator<Item = Point<f32>>) -> Option<Bounds<f32>> {
    points.fold(None, |extent: Option<Bounds<f32>>, at| {
        let (low, high) = match extent {
            Some(extent) => (
                point(extent.origin.x.min(at.x), extent.origin.y.min(at.y)),
                point(
                    extent.bottom_right().x.max(at.x),
                    extent.bottom_right().y.max(at.y),
                ),
            ),
            None => (at, at),
        };
        Some(Bounds::new(low, size(high.x - low.x, high.y - low.y)))
    })
}

/// What branch `name` is known by across reloads while it has no commits.
fn slot_key(name: &FullName) -> SharedString {
    name.as_bstr().to_string().into()
}

/// An empty branch is its lane's, as its first commit will be.
fn empty_kind(empty: &Empty) -> Kind {
    Kind::Lane(empty.lane, CommitState::Local, false)
}

/// Which commits `from` and their ancestors are, not going past those `stop` marks.
fn reach(commits: &[Input], from: Vec<usize>, stop: &[bool]) -> Vec<bool> {
    let mut reached: Vec<bool> = std::iter::repeat_n(false, commits.len()).collect();
    let mut queue = VecDeque::from(from);
    while let Some(at) = queue.pop_front() {
        let (Some(slot), Some(commit)) = (reached.get_mut(at), commits.get(at)) else {
            continue;
        };
        if *slot || stop.get(at).copied().unwrap_or(false) {
            continue;
        }
        *slot = true;
        queue.extend(commit.parents.iter().flatten().copied());
    }
    reached
}

/// What a frame paints, handed to the canvas.
struct Paint {
    scene: Arc<Scene>,
    positions: Rc<Vec<Point<f32>>>,
    grown: Vec<f32>,
    camera: Camera,
    k: f32,
    scale: f32,
    hovered: Option<usize>,
    hovered_edge: Option<usize>,
    pinned: Option<usize>,
    lineage: Rc<HashSet<usize>>,
    ripples: Vec<usize>,
    swaps: Rc<HashSet<usize>>,
    /// How far round the spinner under the oldest commits is, from 0 to 1, while it reads
    /// further back.
    spinner: Option<f32>,
    /// How far the ripples are through, from 0 to 1.
    ripple: f32,
    titles: Rc<RefCell<HashMap<usize, SharedString>>>,
    tags: Rc<RefCell<HashMap<usize, Bounds<f32>>>>,
    theme: Theme,
}

/// The colour of lane `n`, which its commits, edges and branches take.
fn hue(theme: &Theme, lane: usize) -> Hsla {
    // Not purple, which is for what the target has, nor the target's own colour.
    match lane % 3 {
        0 => theme.blue,
        1 => theme.safe,
        _ => theme.warn,
    }
}

impl Kind {
    fn accent(self, theme: &Theme) -> Hsla {
        match self {
            Kind::Lane(_, _, true) => theme.danger,
            // In the target, as the navigator marks it.
            Kind::Lane(_, CommitState::Integrated, false) => theme.purple,
            Kind::Lane(lane, CommitState::Local | CommitState::Pushed, false)
            | Kind::Remote(lane) => hue(theme, lane),
            Kind::Incoming => theme.pop,
            Kind::History => theme.text_3,
        }
    }

    /// The colour of a lit edge, and of a hovered or pinned commit's border, so the two match.
    fn lit(self, theme: &Theme) -> Hsla {
        match self {
            Kind::History => theme.text_2,
            Kind::Lane(..) | Kind::Remote(_) | Kind::Incoming => self.accent(theme),
        }
    }
}

impl Look {
    fn accent(self, theme: &Theme) -> Hsla {
        match self {
            Look::Branch(tint) | Look::CheckedOut(tint) => tint.color(theme),
            Look::Remote(lane) | Look::Worktree(lane) => hue(theme, lane),
            Look::Target | Look::Local => theme.pop,
        }
    }
}

impl Paint {
    fn screen(&self, origin: Point<Pixels>, at: Point<f32>) -> Point<f32> {
        point(
            f32::from(origin.x) + at.x * self.k + self.camera.offset.x,
            f32::from(origin.y) + at.y * self.k + self.camera.offset.y,
        )
    }

    /// How thick edges draw.
    fn edge_width(&self) -> f32 {
        (2. * self.k).clamp(1., 2.5) * self.scale.max(1.)
    }

    /// How thick a lit edge draws, and a hovered or pinned commit's border.
    fn lit_width(&self) -> f32 {
        self.edge_width() * 1.6
    }

    /// How much of commit `ix` has grown in, all of one that isn't new.
    fn grown_of(&self, ix: usize) -> f32 {
        self.grown.get(ix).copied().unwrap_or(1.)
    }

    /// How strongly commit `ix` and its edges show: dimmed when another's lineage is lit.
    fn presence(&self, ix: usize) -> f32 {
        match self.pinned {
            Some(_) if !self.lineage.contains(&ix) => 0.22,
            Some(_) | None => 1.,
        }
    }

    fn paint(&self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        window.paint_quad(fill(bounds, self.theme.bg_2));
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            self.paint_grid(bounds, window);
            self.paint_edges(bounds, window);
            self.paint_nodes(bounds, window, cx);
            self.paint_labels(bounds, window, cx);
            self.paint_spinner(bounds, window);
        });
    }

    /// Dots on a grid that grows coarser as the graph zooms out, so it never crowds.
    fn paint_grid(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        let mut step = 32. * self.k;
        while step < 22. {
            step *= 2.;
        }
        let dot = 1.5 * self.scale;
        let color = self.theme.border_3;
        let (width, height) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
        let start = point(
            self.camera.offset.x.rem_euclid(step),
            self.camera.offset.y.rem_euclid(step),
        );
        let mut y = start.y;
        while y < height {
            let mut x = start.x;
            while x < width {
                let at = point(
                    bounds.origin.x + fixed(x - dot / 2.),
                    bounds.origin.y + fixed(y - dot / 2.),
                );
                let square = Bounds::new(at, size(fixed(dot), fixed(dot)));
                window.paint_quad(fill(square, color).corner_radii(fixed(dot / 2.)));
                x += step;
            }
            y += step;
        }
    }

    fn paint_edges(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        let theme = &self.theme;
        let scene = &self.scene;
        let sizes = Sizes::at(self.k);
        let half = sizes.pill.height / 2.;
        let row = ROW * self.k;
        let spacing = Spacing::at(self.k, sizes);
        let width = self.edge_width();
        let (left, top) = (f32::from(bounds.left()), f32::from(bounds.top()));
        let (right, bottom) = (f32::from(bounds.right()), f32::from(bounds.bottom()));
        // Edges of one colour go in one path, as a path is drawn in one: solid, and dashed for
        // a remote's. The hovered edge goes over the rest, brighter and thicker.
        let mut strokes: Vec<(Hsla, [PathBuilder; 2])> = Vec::new();
        let mut hot: Vec<(Hsla, [PathBuilder; 2])> = Vec::new();
        let origin = self.screen(bounds.origin, point(0., 0.));
        for (ix, edge) in scene.edges.iter().enumerate() {
            let (child, parent) = (edge.child, edge.parent);
            let Some(ends) = edge_ends(edge, &scene.nodes, &self.positions, sizes, self.k, origin)
            else {
                continue;
            };
            let Ends { start, end, .. } = ends;
            // An edge across goes up beside its child, at most a pill's width out.
            if start.x.max(end.x) < left - PILL.width * self.k
                || start.x.min(end.x) > right + PILL.width * self.k
                || end.y > bottom + half
                || start.y < top - half
            {
                continue;
            }
            let on = match edge.route {
                Route::Merge => parent,
                Route::Straight | Route::Fork | Route::Across => child,
            };
            let lit = self.presence(child).min(self.presence(parent));
            let lit = lit * self.grown_of(child).min(self.grown_of(parent));
            let (Some(on), Some(child_node)) = (scene.nodes.get(on), scene.nodes.get(child)) else {
                continue;
            };
            let lights = self.hovered_edge == Some(ix);
            // A pinned commit's lineage takes the lit colour too, as its border does.
            let lineage = self.pinned.is_some()
                && self.lineage.contains(&child)
                && self.lineage.contains(&parent);
            let color = match (on.kind, lights || lineage) {
                (kind, true) => kind.lit(theme),
                (Kind::History, false) => theme.border_1,
                (kind @ (Kind::Lane(..) | Kind::Remote(_) | Kind::Incoming), false) => {
                    kind.accent(theme).opacity(0.85)
                }
            };
            let color = color.opacity(lit);
            let (strokes, width) = match lights {
                true => (&mut hot, self.lit_width()),
                false => (&mut strokes, width),
            };
            if !strokes.iter().any(|(c, _)| *c == color) {
                let line = PathBuilder::stroke(fixed(width));
                let dashed = PathBuilder::stroke(fixed(width))
                    .dash_array(&[fixed(4. * self.scale), fixed(3. * self.scale)]);
                strokes.push((color, [line, dashed]));
            }
            let Some((_, [line, dashed])) = strokes.iter_mut().find(|(c, _)| *c == color) else {
                continue;
            };
            let path = match child_node.kind {
                Kind::Remote(_) => dashed,
                Kind::Lane(..) | Kind::Incoming | Kind::History => line,
            };
            trace(path, start, stretches(edge, ends, spacing));
        }
        // Where the graph stops, a stub fades down towards what it doesn't show.
        let mut stubs = PathBuilder::stroke(fixed(width))
            .dash_array(&[fixed(2. * self.scale), fixed(4. * self.scale)]);
        let mut any = false;
        for (node, at) in scene.nodes.iter().zip(self.positions.iter()) {
            if !node.cut {
                continue;
            }
            let at = self.screen(bounds.origin, *at);
            if at.y < top - row || at.y > bottom + row || at.x < left - 50. || at.x > right + 50. {
                continue;
            }
            stubs.move_to(pixels(point(at.x, at.y + half)));
            stubs.line_to(pixels(point(at.x, at.y + half + row * 0.7)));
            any = true;
        }
        if any && let Ok(stubs) = stubs.build() {
            window.paint_path(stubs, theme.border_2);
        }
        for (color, paths) in strokes {
            for path in paths {
                if let Ok(path) = path.build() {
                    window.paint_path(path, color);
                }
            }
        }
        for (color, paths) in hot {
            for path in paths {
                if let Ok(path) = path.build() {
                    window.paint_path(path, color);
                }
            }
        }
    }

    fn paint_nodes(&self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let theme = &self.theme;
        let sizes = Sizes::at(self.k);
        let (pill, card) = (sizes.opened, sizes.card);
        let (left, top) = (f32::from(bounds.left()), f32::from(bounds.top()));
        let (right, bottom) = (f32::from(bounds.right()), f32::from(bounds.bottom()));
        let title_size = ((TITLE_SIZE * self.k) * 2.).round() / 2.;
        let shows_text = pill > 0.5 && title_size >= 6.;
        let text_alpha = smooth(pill, 0.5, 1.);
        let ui_font = font(theme.ui_font.clone());
        let title_font = Font {
            weight: FontWeight::MEDIUM,
            ..ui_font.clone()
        };
        let lifted = self.lifted();
        // The lit ones last, over the others.
        let order = (0..self.scene.nodes.len())
            .filter(|ix| !lifted.contains(&Some(*ix)))
            .chain(lifted.into_iter().flatten());
        for ix in order {
            let (Some(node), Some(&at)) = (self.scene.nodes.get(ix), self.positions.get(ix)) else {
                continue;
            };
            if node.shape == Shape::Empty {
                continue;
            }
            let centre = self.screen(bounds.origin, at);
            let lift = match lifted.contains(&Some(ix)) {
                true => 1.06,
                false => 1.,
            };
            let grown = self.grown_of(ix);
            let base = sizes.of(node);
            let (width, height) = (base.width * lift * grown, base.height * lift * grown);
            if centre.x + width / 2. < left
                || centre.x - width / 2. > right
                || centre.y + height / 2. < top
                || centre.y - height / 2. > bottom
                || grown <= 0.
            {
                continue;
            }
            let rect = Bounds::new(
                pixels(point(centre.x - width / 2., centre.y - height / 2.)),
                size(fixed(width), fixed(height)),
            );
            let radius = match node.shape {
                Shape::Merge | Shape::Empty => height / 2.,
                Shape::Commit => lerp(height / 2., (9. * self.k).min(height / 2.), card),
            };
            let corners = Corners::all(fixed(radius));
            let presence = self.presence(ix) * grown;
            let accent = node.kind.accent(theme);
            // A commit a reload just brought glows as it lands.
            if self.ripples.contains(&ix) && self.ripple < 1. {
                let glow = 0.7 * (1. - self.ripple);
                window.paint_drop_shadows(
                    rect,
                    corners,
                    &[BoxShadow {
                        color: accent.opacity(glow * presence),
                        offset: point(fixed(0.), fixed(0.)),
                        blur_radius: fixed(18. * self.scale),
                        spread_radius: fixed(1. * self.scale),
                        inset: false,
                    }],
                );
            }
            if self.ripples.contains(&ix) && self.ripple < 1. {
                let grow = ease(self.ripple) * 26. * self.scale;
                let ring = Bounds::new(
                    pixels(point(
                        centre.x - width / 2. - grow,
                        centre.y - height / 2. - grow,
                    )),
                    size(fixed(width + 2. * grow), fixed(height + 2. * grow)),
                );
                let fade = (1. - self.ripple).powi(2) * presence;
                window.paint_quad(quad(
                    ring,
                    Corners::all(fixed(radius + grow)),
                    gpui::transparent_black(),
                    fixed(1.5 * self.scale),
                    accent.opacity(0.8 * fade),
                    gpui::BorderStyle::Solid,
                ));
            }
            if pill <= 0. {
                let color = match node.kind {
                    Kind::History => theme.border_1,
                    Kind::Lane(..) | Kind::Remote(_) | Kind::Incoming => accent,
                };
                window.paint_quad(fill(rect, color.opacity(presence)).corner_radii(corners));
                continue;
            }
            let (border, style) = match node.kind {
                Kind::History => (theme.border_2, gpui::BorderStyle::Solid),
                Kind::Remote(_) => (accent, gpui::BorderStyle::Dashed),
                Kind::Lane(..) | Kind::Incoming => (accent, gpui::BorderStyle::Solid),
            };
            // Hovered or pinned, it takes a lit edge's colour.
            let border = match lifted.contains(&Some(ix)) {
                true => node.kind.lit(theme),
                false => border,
            };
            // From a dot's colour to a pill's ground as it opens.
            let ground = match node.kind {
                Kind::History => theme.border_1,
                Kind::Lane(..) | Kind::Remote(_) | Kind::Incoming => accent,
            };
            window.paint_quad(quad(
                rect,
                corners,
                ground.opacity((1. - pill) * presence),
                fixed(0.),
                gpui::transparent_black(),
                gpui::BorderStyle::Solid,
            ));
            window.paint_quad(quad(
                rect,
                corners,
                theme.bg_1.opacity(pill * presence),
                fixed(0.),
                gpui::transparent_black(),
                gpui::BorderStyle::Solid,
            ));
            let tint = match node.kind {
                Kind::History => 0.,
                Kind::Lane(..) | Kind::Incoming => 0.13,
                Kind::Remote(_) => 0.06,
            };
            window.paint_quad(quad(
                rect,
                corners,
                accent.opacity(tint * pill * presence),
                // Hovered or pinned, its border thickens to a lit edge's width.
                match lifted.contains(&Some(ix)) {
                    true => fixed(self.lit_width()),
                    false => fixed((1.2 * self.scale).max(1.)),
                },
                border.opacity(pill * presence),
                style,
            ));
            let glyph_color = accent.opacity(pill * presence);
            if node.shape == Shape::Merge {
                // In whole pixels, two at a time, so zooming draws it at few sizes.
                let icon = (0.3 * height).round() * 2.;
                if icon >= 4. {
                    let at = Bounds::new(
                        pixels(point(centre.x - icon / 2., centre.y - icon / 2.)),
                        size(fixed(icon), fixed(icon)),
                    );
                    let path = SharedString::from(Icon::BranchMerge.path());
                    let unit = TransformationMatrix::unit();
                    window.paint_svg(at, path, None, unit, glyph_color, cx).ok();
                }
                continue;
            }
            // Its glyph: a ring while only here, filled once pushed, as the navigator's. Up
            // close it keeps to the title's row.
            let head = PILL.height * self.k * lift * grown;
            let head = head.min(height);
            let glyph = 0.3 * head;
            let glyph_x = f32::from(rect.origin.x) + head / 2.;
            let glyph_y = lerp(
                f32::from(rect.center().y),
                f32::from(rect.origin.y) + head / 2.,
                card,
            );
            let dot = Bounds::new(
                pixels(point(glyph_x - glyph / 2., glyph_y - glyph / 2.)),
                size(fixed(glyph), fixed(glyph)),
            );
            let filled = match node.kind {
                Kind::Lane(_, CommitState::Local, _) | Kind::Remote(_) => false,
                Kind::Lane(_, CommitState::Pushed | CommitState::Integrated, _)
                | Kind::Incoming
                | Kind::History => true,
            };
            window.paint_quad(quad(
                dot,
                Corners::all(fixed(glyph / 2.)),
                match filled {
                    true => glyph_color,
                    false => gpui::transparent_black(),
                },
                fixed((1.5 * self.k).clamp(1., 2.)),
                glyph_color,
                gpui::BorderStyle::Solid,
            ));
            if !shows_text {
                continue;
            }
            let text_left = f32::from(dot.right()) + 6. * self.k;
            let inner = Bounds::new(
                point(fixed(text_left), rect.origin.y),
                size(
                    fixed((f32::from(rect.right()) - 10. * self.k - text_left).max(0.)),
                    rect.size.height,
                ),
            );
            let title = self.title(ix, &title_font, window);
            let color = match node.kind {
                Kind::History => theme.text_2,
                Kind::Lane(..) | Kind::Remote(_) | Kind::Incoming => theme.text_1,
            };
            let line_height = title_size * 1.3;
            let title_top = lerp(
                f32::from(rect.center().y) - line_height / 2.,
                f32::from(rect.origin.y) + (head - line_height) / 2.,
                card,
            );
            let run = TextRun {
                len: title.len(),
                font: title_font.clone(),
                color: color.opacity(text_alpha * smooth(grown, 0.7, 1.) * self.presence(ix)),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let shaped = (window.text_system()).shape_line(title, fixed(title_size), &[run], None);
            window.with_content_mask(Some(ContentMask { bounds: inner }), |window| {
                shaped
                    .paint(
                        point(fixed(text_left), fixed(title_top)),
                        fixed(line_height),
                        TextAlign::Left,
                        None,
                        window,
                        cx,
                    )
                    .ok();
                // Once the pill has grown tall enough for it, not faded in while it grows.
                let meta_size = ((10.5 * self.k) * 2.).round() / 2.;
                let meta_bottom = title_top + line_height + meta_size * 1.3;
                if card > 0. && meta_bottom <= f32::from(rect.bottom()) - 4. * self.k {
                    let meta: SharedString =
                        format!("{} · {} · {}", node.id.short(), node.author, ago(node.time))
                            .into();
                    let run = TextRun {
                        len: meta.len(),
                        font: ui_font.clone(),
                        color: theme.text_3.opacity(presence),
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    let shaped =
                        (window.text_system()).shape_line(meta, fixed(meta_size), &[run], None);
                    shaped
                        .paint(
                            point(fixed(text_left), fixed(title_top + line_height)),
                            fixed(meta_size * 1.3),
                            TextAlign::Left,
                            None,
                            window,
                            cx,
                        )
                        .ok();
                }
            });
        }
    }

    /// Commit `ix`'s title, cut with an ellipsis to fit its pill.
    fn title(&self, ix: usize, font: &Font, window: &Window) -> SharedString {
        if let Some(title) = self.titles.borrow().get(&ix) {
            return title.clone();
        }
        let Some(title) = self.scene.nodes.get(ix).map(|node| node.title.clone()) else {
            return SharedString::default();
        };
        let size = TITLE_SIZE * self.scale;
        // Past the glyph, as `paint_nodes` places it, to the pill's right padding.
        let room = (PILL.width - 0.65 * PILL.height - 6. - 10.) * self.scale;
        let run = TextRun {
            len: title.len(),
            font: font.clone(),
            color: self.theme.text_1,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let shaped = (window.text_system()).shape_line(title.clone(), fixed(size), &[run], None);
        let cut = match f32::from(shaped.width) <= room {
            true => title,
            false => {
                let at = (shaped.index_for_x(fixed(room - size))).unwrap_or(0);
                let at = (0..=at)
                    .rev()
                    .find(|at| title.is_char_boundary(*at))
                    .unwrap_or(0);
                format!("{}…", title.get(..at).unwrap_or_default().trim_end()).into()
            }
        };
        self.titles.borrow_mut().insert(ix, cut.clone());
        cut
    }

    /// A short arc going round a faint ring under the oldest commits, while it reads further
    /// back, below them however far it's zoomed out.
    fn paint_spinner(&self, bounds: Bounds<Pixels>, window: &mut Window) {
        let Some(turn) = self.spinner else {
            return;
        };
        let oldest = self.scene.extent.origin.y + self.scene.extent.size.height;
        let below = self.screen(bounds.origin, point(0., oldest));
        let (radius, width) = (7. * self.scale, 1.5 * self.scale);
        let centre = point(below.x, below.y + (ROW / 2. * self.k).max(24. * self.scale));
        window.paint_quad(quad(
            Bounds::new(
                pixels(point(centre.x - radius, centre.y - radius)),
                size(fixed(2. * radius), fixed(2. * radius)),
            ),
            Corners::all(fixed(radius)),
            gpui::transparent_black(),
            fixed(width),
            self.theme.text_3.opacity(0.25),
            gpui::BorderStyle::Solid,
        ));
        let on = |angle: f32| {
            let inner = radius - width / 2.;
            pixels(point(
                centre.x + inner * angle.cos(),
                centre.y + inner * angle.sin(),
            ))
        };
        let start = turn * std::f32::consts::TAU;
        let mut arc = PathBuilder::stroke(fixed(width));
        arc.move_to(on(start));
        let inner = fixed(radius - width / 2.);
        arc.arc_to(point(inner, inner), fixed(0.), false, true, on(start + 1.6));
        if let Ok(path) = arc.build() {
            window.paint_path(path, self.theme.text_2);
        }
    }

    /// The commits drawn raised: the pinned and hovered ones, and the hovered edge's ends.
    fn lifted(&self) -> [Option<usize>; 4] {
        let edge = (self.hovered_edge).and_then(|edge| self.scene.edges.get(edge));
        [
            self.pinned,
            self.hovered,
            edge.map(|edge| edge.child),
            edge.map(|edge| edge.parent),
        ]
    }

    /// The branches, worktrees and target beside the commits they name, readable from far.
    fn paint_labels(&self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let theme = &self.theme;
        let mut tags = self.tags.borrow_mut();
        tags.clear();
        let fade = smooth(self.k, 0.06, 0.12);
        if fade <= 0. {
            return;
        }
        let lifted = self.lifted();
        let sizes = Sizes::at(self.k);
        let pill = sizes.opened;
        let text_size =
            ((11. * self.k).clamp(9. * self.scale, 12.5 * self.scale) * 2.).round() / 2.;
        let height = text_size * 1.75;
        let pad = text_size * 0.65;
        let label_font = Font {
            weight: FontWeight::SEMIBOLD,
            ..font(theme.ui_font.clone())
        };
        let (right, bottom) = (f32::from(bounds.right()), f32::from(bounds.bottom()));
        let (left, top) = (f32::from(bounds.left()), f32::from(bounds.top()));
        for (ix, (node, at)) in self
            .scene
            .nodes
            .iter()
            .zip(self.positions.iter())
            .enumerate()
        {
            if node.labels.is_empty() {
                continue;
            }
            let centre = self.screen(bounds.origin, *at);
            let reach = 2. * height;
            if centre.y < top - reach || centre.y > bottom + reach {
                continue;
            }
            // A new commit's tags grow in with it, rising out of it, but a branch's stay as its
            // first commit comes, rising over it, or its last goes, falling into its place.
            let grown = self.grown_of(ix);
            let swapped = self.swaps.contains(&ix);
            let presence = match swapped {
                true => self.presence(ix) * fade,
                false => self.presence(ix) * grown * fade,
            };
            let base = sizes.of(node);
            let gap = 4. * self.scale;
            let shaped: Vec<_> = (node.labels.iter())
                .map(|label| {
                    let accent = label.look.accent(theme);
                    let (ground, border, text) = match label.look {
                        Look::Branch(_) | Look::CheckedOut(_) | Look::Target | Look::Local => {
                            (accent, accent, theme.bg_1)
                        }
                        Look::Remote(_) => (gpui::transparent_black(), accent, accent),
                        Look::Worktree(_) => (accent.opacity(0.16), accent.opacity(0.5), accent),
                    };
                    let run = TextRun {
                        len: label.text.len(),
                        font: label_font.clone(),
                        color: text,
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    };
                    let line = (window.text_system()).shape_line(
                        label.text.clone(),
                        fixed(text_size),
                        &[run],
                        None,
                    );
                    let glyph = match label.look.in_worktree() {
                        true => text_size.round() + pad / 2.,
                        false => 0.,
                    };
                    let width = f32::from(line.width) + 2. * pad + glyph;
                    (label, line, width, ground, border, text, glyph)
                })
                .collect();
            // In a row: centred over a pill, and beside a dot far away. A branch with no
            // commits has only its tags, where its commit would be.
            let row: f32 = shaped.iter().map(|tag| tag.2 + gap).sum::<f32>() - gap;
            let beside = point(
                centre.x + base.width / 2. + 6. * self.scale,
                centre.y - height / 2.,
            );
            let above = point(
                centre.x - row / 2.,
                lerp(
                    centre.y - height / 2.,
                    centre.y - base.height / 2. - 4. * self.scale - height,
                    grown,
                ),
            );
            let (mut x, y) = match node.shape {
                Shape::Commit | Shape::Merge => {
                    (lerp(beside.x, above.x, pill), lerp(beside.y, above.y, pill))
                }
                Shape::Empty => {
                    let over = match swapped {
                        true => sizes.pill.height / 2. + 4. * self.scale + height / 2.,
                        false => 0.,
                    };
                    (
                        centre.x - row / 2.,
                        centre.y - height / 2. - over * (1. - grown),
                    )
                }
            };
            if node.shape == Shape::Empty {
                tags.insert(ix, Bounds::new(point(x - left, y - top), size(row, height)));
                // Hovered or pinned, it's ringed in a lit edge's colour and width, a little
                // way out so the ring shows against a tag of its colour.
                if lifted.contains(&Some(ix)) {
                    let width = self.lit_width();
                    let out = width + 2. * self.scale;
                    let ring = Bounds::new(
                        pixels(point(x - out, y - out)),
                        size(fixed(row + 2. * out), fixed(height + 2. * out)),
                    );
                    window.paint_quad(quad(
                        ring,
                        Corners::all(fixed(height / 2. + out)),
                        gpui::transparent_black(),
                        fixed(width),
                        node.kind.lit(theme).opacity(presence),
                        gpui::BorderStyle::Solid,
                    ));
                }
            }
            for (label, shaped, width, ground, border, text, glyph) in shaped {
                if x + width >= left && x <= right && y + height >= top && y <= bottom {
                    let rect = Bounds::new(pixels(point(x, y)), size(fixed(width), fixed(height)));
                    // On the page's ground first, so a tinted tag hides the edges behind it.
                    window.paint_quad(fill(rect, theme.bg_2).corner_radii(fixed(height / 2.)));
                    window.paint_quad(quad(
                        rect,
                        Corners::all(fixed(height / 2.)),
                        ground,
                        fixed(1.),
                        border,
                        match label.look {
                            Look::Remote(_) => gpui::BorderStyle::Dashed,
                            Look::Branch(_)
                            | Look::Worktree(_)
                            | Look::CheckedOut(_)
                            | Look::Target
                            | Look::Local => gpui::BorderStyle::Solid,
                        },
                    ));
                    // The worktree glyph, in whole pixels so zooming draws it at few sizes.
                    if glyph > 0. {
                        let icon = text_size.round();
                        let at = Bounds::new(
                            pixels(point(x + pad * 0.8, y + (height - icon) / 2.)),
                            size(fixed(icon), fixed(icon)),
                        );
                        let path = SharedString::from(Icon::FolderCopy.path());
                        let unit = TransformationMatrix::unit();
                        window.paint_svg(at, path, None, unit, text, cx).ok();
                    }
                    shaped
                        .paint(
                            pixels(point(x + pad + glyph, y + (height - text_size * 1.3) / 2.)),
                            fixed(text_size * 1.3),
                            TextAlign::Left,
                            None,
                            window,
                            cx,
                        )
                        .ok();
                    // Faded under the ground, as a branch's text is the ground's colour.
                    if presence < 1. {
                        window.paint_quad(
                            fill(rect, theme.bg_2.opacity(1. - presence))
                                .corner_radii(fixed(height / 2.)),
                        );
                    }
                }
                x += width + gap;
            }
        }
    }
}

fn middle(bounds: &Bounds<f32>) -> Point<f32> {
    point(
        bounds.origin.x + bounds.size.width / 2.,
        bounds.origin.y + bounds.size.height / 2.,
    )
}

fn pixels(at: Point<f32>) -> Point<Pixels> {
    point(fixed(at.x), fixed(at.y))
}

/// Where an edge runs on screen: from its parent's top to its child's bottom, and how far
/// those are from their middles, a merge's circle being shorter than a pill.
#[derive(Debug, Clone, Copy)]
struct Ends {
    start: Point<f32>,
    end: Point<f32>,
    halves: (f32, f32),
}

/// Where `edge` runs, with the layout's 0 drawn at `origin` and a unit `k` pixels.
fn edge_ends(
    edge: &layout::Edge,
    nodes: &[Node],
    positions: &[Point<f32>],
    sizes: Sizes,
    k: f32,
    origin: Point<f32>,
) -> Option<Ends> {
    let (parent, child) = (nodes.get(edge.parent)?, nodes.get(edge.child)?);
    let (from, to) = (positions.get(edge.parent)?, positions.get(edge.child)?);
    let halves = (sizes.of(parent).height / 2., sizes.of(child).height / 2.);
    Some(Ends {
        start: point(origin.x + from.x * k, origin.y + from.y * k - halves.0),
        end: point(origin.x + to.x * k, origin.y + to.y * k + halves.1),
        halves,
    })
}

/// How rows draw at some zoom: the gap between one row's commits and the next's, half a
/// pill's height, and pixels a unit of the layout takes.
#[derive(Debug, Clone, Copy)]
struct Spacing {
    gap: f32,
    half: f32,
    k: f32,
}

impl Spacing {
    fn at(k: f32, sizes: Sizes) -> Spacing {
        Spacing {
            gap: (ROW * k - sizes.pill.height).max(0.),
            half: sizes.pill.height / 2.,
            k,
        }
    }

    /// How high a turn on `track` crosses the gap whose bottom is at `bottom`: the middle
    /// without one.
    fn crossing(self, bottom: f32, track: Option<Track>) -> f32 {
        let (at, of) = track.map_or((0, 1), |track| (track.at, track.of));
        bottom - self.gap * f32::from(at + 1) / f32::from(of + 1)
    }
}

/// A stretch of an edge: straight, or a turn into another column across a gap at a height, as
/// [`turn`] draws it.
#[derive(Debug, Clone, Copy)]
enum Stretch {
    Line(Point<f32>),
    Turn(Point<f32>, f32),
}

/// What `edge` runs through from its parent's top to its child's bottom, each to where it
/// ends.
fn stretches(edge: &layout::Edge, ends: Ends, spacing: Spacing) -> [Option<Stretch>; 3] {
    let Ends { start, end, halves } = ends;
    // The bottom of the gap above the parent's row, and of the one below the child's.
    let above_parent = start.y + halves.0 - spacing.half;
    let below_child = end.y - halves.1 + spacing.half + spacing.gap;
    let [out, into] = edge.tracks;
    match edge.route {
        Route::Straight => [Some(Stretch::Line(end)), None, None],
        Route::Fork => {
            // Out of the parent and over into the child's column a row up.
            let turned = point(end.x, (above_parent - spacing.gap).max(end.y));
            let rest = (turned.y > end.y).then_some(Stretch::Line(end));
            let across = spacing.crossing(above_parent, out);
            [Some(Stretch::Turn(turned, across)), rest, None]
        }
        Route::Merge => {
            // Up the parent's column to a row below the child, then over into it.
            let turned = point(start.x, below_child.min(start.y));
            let up = (turned.y < start.y).then_some(Stretch::Line(turned));
            let across = spacing.crossing(below_child, into);
            [up, Some(Stretch::Turn(end, across)), None]
        }
        Route::Across if into.is_none() => {
            let across = spacing.crossing(above_parent, out);
            [Some(Stretch::Turn(end, across)), None, None]
        }
        // Over into the gap beside the child towards the parent, up it and over into the child,
        // clear of both lines.
        Route::Across => {
            let between = end.x + edge.aside * spacing.k;
            let out_to = point(between, above_parent - spacing.gap);
            [
                Some(Stretch::Turn(out_to, spacing.crossing(above_parent, out))),
                Some(Stretch::Line(point(between, below_child.min(out_to.y)))),
                Some(Stretch::Turn(end, spacing.crossing(below_child, into))),
            ]
        }
    }
}

fn trace(path: &mut PathBuilder, start: Point<f32>, stretches: [Option<Stretch>; 3]) {
    path.move_to(pixels(start));
    let mut from = start;
    for stretch in stretches.into_iter().flatten() {
        match stretch {
            Stretch::Line(to) => {
                path.line_to(pixels(to));
                from = to;
            }
            Stretch::Turn(to, across) => {
                for piece in turn(from, to, across) {
                    match piece {
                        Piece::Line(to) => path.line_to(pixels(to)),
                        Piece::Cubic(a, b, to) => {
                            path.cubic_bezier_to(pixels(to), pixels(a), pixels(b))
                        }
                    }
                }
                from = to;
            }
        }
    }
}

/// A piece of a turn: a line to a point, or a cubic curve to one through two control points.
#[derive(Debug, Clone, Copy)]
enum Piece {
    Line(Point<f32>),
    Cubic(Point<f32>, Point<f32>, Point<f32>),
}

/// A turn from `from` up into `to`'s column, crossing at height `across`: up, round a corner,
/// across, round another and on up, as a rounded rectangle's edge runs. Each corner is as
/// round as the room above or below the crossing and half the width allow, so a long way
/// across in one row's height doesn't kink.
fn turn(from: Point<f32>, to: Point<f32>, across: f32) -> [Piece; 5] {
    // How far along its tangents a quarter circle's control points sit, of its radius.
    const KAPPA: f32 = 0.5523;
    let side = match to.x < from.x {
        true => -1.,
        false => 1.,
    };
    let across = across.clamp(to.y.min(from.y), from.y.max(to.y));
    let half_way = (to.x - from.x).abs() / 2.;
    let (low, high) = (
        (from.y - across).min(half_way).max(0.),
        (across - to.y).min(half_way).max(0.),
    );
    let (a, b) = (
        point(from.x, across + low),
        point(from.x + side * low, across),
    );
    let (c, d) = (
        point(to.x - side * high, across),
        point(to.x, across - high),
    );
    let (bend_low, bend_high) = (KAPPA * low, KAPPA * high);
    [
        Piece::Line(a),
        Piece::Cubic(
            point(a.x, a.y - bend_low),
            point(b.x - side * bend_low, b.y),
            b,
        ),
        Piece::Line(c),
        Piece::Cubic(
            point(c.x + side * bend_high, c.y),
            point(d.x, d.y + bend_high),
            d,
        ),
        Piece::Line(to),
    ]
}

/// How far `at` is from an edge from `start` through `stretches`.
fn distance(at: Point<f32>, start: Point<f32>, stretches: [Option<Stretch>; 3]) -> f32 {
    // A corner is near enough a line between points along it for pointing at.
    const STEPS: u16 = 8;
    let mut nearest = f32::MAX;
    let mut from = start;
    for stretch in stretches.into_iter().flatten() {
        from = match stretch {
            Stretch::Line(to) => {
                nearest = nearest.min(to_segment(at, from, to));
                to
            }
            Stretch::Turn(to, across) => {
                let mut last = from;
                for piece in turn(from, to, across) {
                    last = match piece {
                        Piece::Line(next) => {
                            nearest = nearest.min(to_segment(at, last, next));
                            next
                        }
                        Piece::Cubic(a, b, end) => {
                            let mut along = last;
                            for step in 1..=STEPS {
                                let t = f32::from(step) / f32::from(STEPS);
                                let next = bezier([last, a, b, end], t);
                                nearest = nearest.min(to_segment(at, along, next));
                                along = next;
                            }
                            end
                        }
                    };
                }
                to
            }
        };
    }
    nearest
}

/// The point `t` of the way along the cubic curve through `points`.
fn bezier(points: [Point<f32>; 4], t: f32) -> Point<f32> {
    let u = 1. - t;
    let weights = [u * u * u, 3. * u * u * t, 3. * u * t * t, t * t * t];
    let (x, y) = (points.iter().zip(weights)).fold((0., 0.), |(x, y), (point, weight)| {
        (x + point.x * weight, y + point.y * weight)
    });
    point(x, y)
}

/// How far `at` is from the line from `from` to `to`.
fn to_segment(at: Point<f32>, from: Point<f32>, to: Point<f32>) -> f32 {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let length = dx * dx + dy * dy;
    let t = match length > 0. {
        true => (((at.x - from.x) * dx + (at.y - from.y) * dy) / length).clamp(0., 1.),
        false => 0.,
    };
    (at.x - from.x - t * dx).hypot(at.y - from.y - t * dy)
}

impl Render for GraphView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        self.scale = theme.scale;
        let Some(scene) = self.scene.clone() else {
            let (title, detail) = match &self.failed {
                Some(err) => ("Couldn't read the graph", err.clone()),
                None => ("Reading the graph", "Its commits are on their way".into()),
            };
            return div()
                .size_full()
                .bg(theme.bg_2)
                .child(empty_state(None, title, detail, &theme, cx))
                .into_any_element();
        };
        if scene.nodes.is_empty() {
            return div()
                .size_full()
                .bg(theme.bg_2)
                .child(empty_state(
                    None,
                    "No commits yet",
                    "The graph grows as commits are made",
                    &theme,
                    cx,
                ))
                .into_any_element();
        }
        let now = Instant::now();
        // The canvas's size is known once it's drawn, so the first frame only measures it.
        let measured = self.bounds.get().size.width > fixed(0.);
        if !self.placed && measured {
            self.camera = self.opening(&scene);
            self.placed = true;
        }
        // The oldest commits read in view, it reads further back.
        let oldest = scene.extent.origin.y + scene.extent.size.height;
        let bottom = (self.camera)
            .at(
                point(0., f32::from(self.bounds.get().size.height)),
                self.scale,
            )
            .y;
        if scene.more
            && self.placed
            && bottom + ROW >= oldest
            && self.history == scene.history
            && self.deepened != Some(self.camera)
        {
            self.history += HISTORY;
            self.deepening = Some(now);
            self.load(cx);
        }
        let (positions, grown, moving) = self.frame(&scene, now);
        let hover_fade = smooth(now.duration_since(self.hovered_at).as_secs_f32(), 0.08, 0.2);
        let spinning = self.deepening.is_some();
        if moving || spinning || !self.placed || (self.hovered.is_some() && hover_fade < 1.) {
            window.request_animation_frame();
        }
        self.drawn = Rc::new(positions);
        let pinned = (self.pinned.as_ref()).and_then(|key| scene.keys.get(key).copied());
        let mut paint = Paint {
            scene: scene.clone(),
            positions: self.drawn.clone(),
            grown,
            camera: self.camera,
            k: self.k(),
            scale: self.scale,
            hovered: self.hovered,
            hovered_edge: self.hovered_edge,
            pinned,
            lineage: self.lineage.clone(),
            ripples: self.ripples.0.clone(),
            swaps: self.swaps.clone(),
            spinner: (self.deepening)
                .map(|at| (now.duration_since(at).as_secs_f32() / SPIN.as_secs_f32()).fract()),
            ripple: self.ripple_age(now).min(1.),
            titles: self.titles.clone(),
            tags: self.tags.clone(),
            theme: theme.clone(),
        };
        let placed = self.placed;
        let cursor = match (&self.drag, self.hovered) {
            (Some(drag), _) if drag.moved => CursorStyle::ClosedHand,
            (_, Some(_)) => CursorStyle::PointingHand,
            (Some(_) | None, None) => CursorStyle::Arrow,
        };
        let view = cx.entity().downgrade();
        let bounds = self.bounds.clone();
        let graph = canvas(
            move |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
            move |area, hitbox, window, cx| {
                let was = bounds.replace(area);
                // Resized, as when the navigator opens or closes, it keeps what was in its
                // middle there, from this frame on.
                let grown = point(
                    f32::from(area.size.width - was.size.width),
                    f32::from(area.size.height - was.size.height),
                );
                if placed && was.size.width > fixed(0.) && grown != point(0., 0.) {
                    paint.camera = paint.camera.panned(point(grown.x / 2., grown.y / 2.));
                    // The cards were placed before it knew, so they go again.
                    view.update(cx, |this, cx| {
                        this.resized(grown);
                        cx.notify();
                    })
                    .ok();
                }
                if placed {
                    paint.paint(area, window, cx);
                }
                window.set_cursor_style(cursor, &hitbox);
                listen(&view, &hitbox, window);
            },
        )
        .size_full();
        let k = self.k();
        let offset = self.camera.offset;
        // The pinned commit's card, and the hovered one's while it's another.
        let cards = [pinned, self.hovered.filter(|ix| Some(*ix) != pinned)]
            .into_iter()
            .flatten()
            .filter(|_| placed)
            .filter_map(|ix| {
                let (at, node) = (self.drawn.get(ix)?, scene.nodes.get(ix)?);
                let (centre, node_box) = match self.tags.borrow().get(&ix) {
                    Some(row) => (middle(row), row.size),
                    None => (
                        point(at.x * k + offset.x, at.y * k + offset.y),
                        Sizes::at(k).of(node),
                    ),
                };
                let fade = match Some(ix) == pinned {
                    true => 1.,
                    false => hover_fade,
                };
                let pinned = Some(ix) == pinned;
                let view = self.bounds.get().size;
                Some(card(node, centre, node_box, view, fade, pinned, &theme))
            });
        let zoom = self.camera.zoom;
        let target = (scene.nodes.iter())
            .flat_map(|node| &node.labels)
            .find(|label| label.look == Look::Target)
            .map(|label| label.text.clone());
        let controls = h_flex()
            .id("graph-controls")
            .absolute()
            .right(dp(12.))
            .bottom(dp(12.))
            .occlude()
            .gap(dp(2.))
            .p(dp(2.))
            .rounded(dp(8.))
            .bg(theme.bg_1)
            .border_1()
            .border_color(theme.border_2)
            .shadow_sm()
            .child(
                button("graph-workspace", "Workspace")
                    .style(ButtonStyle::Ghost)
                    .small()
                    .tooltip("Your lanes and what they start from")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        if let Some(scene) = this.scene.clone() {
                            let to = this.on_workspace(&scene);
                            this.glide_to(to, cx);
                        }
                    })),
            )
            .child(
                button(
                    "graph-target",
                    target.clone().unwrap_or_else(|| "Target".into()),
                )
                .style(ButtonStyle::Ghost)
                .small()
                .tooltip("The target's newest commit")
                .disabled_because(target.is_none().then_some("There's no target"))
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    if let Some(to) = (this.scene.clone()).and_then(|scene| this.on_target(&scene))
                    {
                        this.glide_to(to, cx);
                    }
                })),
            )
            .child(div().w(fixed(1.)).h(dp(16.)).mx(dp(2.)).bg(theme.border_2))
            .child(
                icon_button("graph-zoom-out", Icon::Minus)
                    .tooltip("Zoom out")
                    .on_click(
                        cx.listener(|this, _: &ClickEvent, _, cx| this.zoom_by(1. / 1.4, cx)),
                    ),
            )
            .child(
                button("graph-zoom-reset", format!("{}%", (zoom * 100.).round()))
                    .style(ButtonStyle::Ghost)
                    .small()
                    .tooltip("Back to 100%")
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.zoom_by(1. / this.camera.zoom, cx)
                    })),
            )
            .child(
                icon_button("graph-zoom-in", Icon::Plus)
                    .tooltip("Zoom in")
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.zoom_by(1.4, cx))),
            );
        div()
            .relative()
            .size_full()
            .overflow_hidden()
            .bg(theme.bg_2)
            .child(graph)
            .children(cards)
            .child(controls)
            .into_any_element()
    }
}

/// The pointer's events on the canvas, `hitbox`, to `view`: presses, pans and zooms over it,
/// and moves and lets go anywhere, as a drag goes on outside it.
fn listen(view: &WeakEntity<GraphView>, hitbox: &Hitbox, window: &mut Window) {
    window.on_mouse_event({
        let (view, hitbox) = (view.clone(), hitbox.clone());
        move |event: &MouseDownEvent, phase, window, cx| {
            if phase == DispatchPhase::Bubble
                && event.button == MouseButton::Left
                && hitbox.is_hovered(window)
            {
                view.update(cx, |this, cx| this.pressed(event, cx)).ok();
            }
        }
    });
    window.on_mouse_event({
        let (view, hitbox) = (view.clone(), hitbox.clone());
        move |event: &MouseMoveEvent, phase, window, cx| {
            if phase == DispatchPhase::Bubble {
                let over = hitbox.is_hovered(window);
                view.update(cx, |this, cx| this.moved_to(event, over, cx))
                    .ok();
            }
        }
    });
    window.on_mouse_event({
        let view = view.clone();
        move |event: &MouseUpEvent, phase, _, cx| {
            if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
                view.update(cx, |this, cx| this.released(cx)).ok();
            }
        }
    });
    window.on_mouse_event({
        let (view, hitbox) = (view.clone(), hitbox.clone());
        move |event: &ScrollWheelEvent, phase, window, cx| {
            // Over the pinned card too, which passes them through.
            if phase == DispatchPhase::Bubble && hitbox.should_handle_scroll(window) {
                view.update(cx, |this, cx| this.scrolled(event, cx)).ok();
                cx.stop_propagation();
            }
        }
    });
    window.on_mouse_event({
        let (view, hitbox) = (view.clone(), hitbox.clone());
        move |event: &PinchEvent, phase, window, cx| {
            if phase == DispatchPhase::Bubble && hitbox.should_handle_scroll(window) {
                view.update(cx, |this, cx| this.pinched(event, cx)).ok();
                cx.stop_propagation();
            }
        }
    });
}

/// Commit `node`'s card, beside it at `centre` in the canvas, `node_box` big, within `view`.
fn card(
    node: &Node,
    centre: Point<f32>,
    node_box: Size<f32>,
    view: Size<Pixels>,
    fade: f32,
    pinned: bool,
    theme: &Theme,
) -> AnyElement {
    const WIDTH: f32 = 340.;
    let scale = theme.scale;
    let (view_width, view_height) = (f32::from(view.width), f32::from(view.height));
    let width = WIDTH * scale;
    let right_of = centre.x + node_box.width / 2. + 14. * scale;
    let left = match right_of + width > view_width - 12. {
        true => centre.x - node_box.width / 2. - 14. * scale - width,
        false => right_of,
    };
    let top = (centre.y - node_box.height / 2.).clamp(12., (view_height - 180. * scale).max(12.));
    let end = node.body.floor_char_boundary(1200);
    let body = reflowed(node.body.get(..end).unwrap_or_default());
    let by = match &node.agent {
        Some(agent) => format!("{} with {}", node.author, agent.name),
        None => node.author.to_string(),
    };
    let tags = node.labels.iter().map(|label| {
        let accent = label.look.accent(theme);
        div()
            .px(dp(6.))
            .rounded(dp(9.))
            .text_size(dp(11.))
            .font_weight(FontWeight::SEMIBOLD)
            .flex()
            .items_center()
            .gap(dp(3.))
            .map(|el| match label.look {
                Look::Branch(_) | Look::CheckedOut(_) | Look::Target | Look::Local => {
                    el.bg(accent).text_color(theme.bg_1)
                }
                Look::Remote(_) | Look::Worktree(_) => {
                    el.border_1().border_color(accent).text_color(accent)
                }
            })
            .when(label.look.in_worktree(), |el| {
                let color = match label.look {
                    Look::CheckedOut(_) => theme.bg_1,
                    Look::Branch(_)
                    | Look::Remote(_)
                    | Look::Worktree(_)
                    | Look::Target
                    | Look::Local => accent,
                };
                el.child(small_icon(Icon::FolderCopy).text_color(color))
            })
            .child(label.text.clone())
    });
    let what = match node.kind {
        Kind::Lane(_, CommitState::Local, _) => "Not pushed",
        Kind::Lane(_, CommitState::Pushed, _) => "Pushed",
        Kind::Lane(_, CommitState::Integrated, _) => "In the target",
        Kind::Remote(_) => "Only on the remote",
        Kind::Incoming => "New on the target",
        Kind::History => "History",
    };
    // An empty branch has no commit to tell of, only its tags.
    let (what, commit) = match node.shape {
        Shape::Commit | Shape::Merge => (what, true),
        Shape::Empty => (NO_COMMITS, false),
    };
    let header = h_flex()
        .gap(dp(10.))
        .items_start()
        .child(byline_avatar(
            &node.author,
            &node.email,
            node.agent.as_ref(),
            24.,
            theme,
        ))
        .child(
            v_flex()
                .min_w_0()
                .flex_grow(1.)
                .gap(dp(2.))
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.text_1)
                        .child(node.title.clone()),
                )
                .child(
                    div()
                        .text_size(dp(12.))
                        .text_color(theme.text_3)
                        .child(format!("{by} · {}", ago(node.time))),
                ),
        );
    popover(theme)
        .absolute()
        .left(fixed(left.max(12.)))
        .top(fixed(top))
        .when(commit, |el| el.w(dp(WIDTH)))
        .p(dp(12.))
        .gap(dp(8.))
        .opacity(fade)
        .when(pinned, |el| {
            // It takes clicks, not what's under it, but pans and zooms go on through it.
            el.block_mouse_except_scroll()
                .border_color(node.kind.accent(theme))
        })
        .when(commit, |el| el.child(header))
        .when(!body.is_empty(), |el| {
            el.child(
                div()
                    .text_size(dp(12.))
                    .text_color(theme.text_2)
                    .max_h(dp(140.))
                    .overflow_hidden()
                    .child(body),
            )
        })
        .child(
            h_flex()
                .flex_wrap()
                .gap(dp(6.))
                .items_center()
                .children(tags)
                .child(
                    div()
                        .text_size(dp(11.))
                        .text_color(theme.text_3)
                        .child(what),
                )
                .when(commit, |el| {
                    el.child(
                        div()
                            .text_size(dp(11.))
                            .font_family(theme.mono_font.clone())
                            .text_color(theme.text_3)
                            .child(node.id.short()),
                    )
                }),
        )
        .into_any_element()
}

/// `body` without the line breaks a commit message wraps its paragraphs at, so the card wraps
/// it at its own width. Blank lines, list items and indented lines keep theirs.
fn reflowed(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let mut continues = false;
    for line in body.lines() {
        let line = line.trim_end();
        let own = line.is_empty() || line.starts_with(char::is_whitespace) || is_item(line);
        if !out.is_empty() {
            out.push(match continues && !own {
                true => ' ',
                false => '\n',
            });
        }
        out.push_str(line);
        continues = !line.is_empty();
    }
    out
}

/// Whether `line` starts a list's item, as `- `, `* ` or `1. ` do.
fn is_item(line: &str) -> bool {
    let numbered = (line.split_once(". "))
        .is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
    numbered
        || ["- ", "* ", "+ "]
            .iter()
            .any(|bullet| line.starts_with(bullet))
}

#[cfg(test)]
mod tests {
    use gpui::point;

    use super::{Camera, glided, names, reflowed};

    #[test]
    fn a_glide_looks_along_a_straight_line() {
        let middle = point(400., 300.);
        let from = Camera {
            offset: point(-600., 900.),
            zoom: 0.25,
        };
        let to = Camera {
            offset: point(1200., -300.),
            zoom: 2.,
        };
        let (a, b) = (from.at(middle, 2.), to.at(middle, 2.));
        for t in [0.25, 0.5, 0.75] {
            let at = glided(from, to, t, middle, 2.).at(middle, 2.);
            let along = point(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
            assert!(
                (at.x - along.x).abs() < 1e-3 && (at.y - along.y).abs() < 1e-3,
                "the middle of the view is {t} of the way along the line, not off it"
            );
        }
        let end = glided(from, to, 1., middle, 2.);
        assert!(
            (end.offset.x - to.offset.x).abs() < 1e-3 && (end.zoom - to.zoom).abs() < 1e-6,
            "it ends where it was going"
        );
    }

    #[test]
    fn a_branch_names_the_worktree_named_after_it() {
        assert!(
            names("dp/but-gpui-graph", "but-gpui-graph"),
            "the worktree's name is the branch's after its prefix"
        );
        assert!(
            names("dp/graph-viz", "dp-graph-viz"),
            "GitButler names a worktree for its branch with its slashes as dashes"
        );
    }

    #[test]
    fn a_worktree_named_otherwise_keeps_its_own_tag() {
        assert!(
            !names("dp/graph-viz", "scratch"),
            "the branch's name doesn't say where it's checked out"
        );
    }

    #[test]
    fn a_wrapped_paragraph_reads_as_one_line() {
        let body = "Hiding the navigator on a page with nothing for the keys, as a checkout\nwith no changes, leaves them on the detail.";
        assert_eq!(
            reflowed(body),
            "Hiding the navigator on a page with nothing for the keys, as a checkout with no changes, leaves them on the detail.",
            "the card wraps it at its own width"
        );
    }

    #[test]
    fn paragraphs_and_lists_keep_their_breaks() {
        let body = "First paragraph\nwrapped.\n\n- one item\n- another\n  indented\n1. numbered";
        assert_eq!(
            reflowed(body),
            "First paragraph wrapped.\n\n- one item\n- another\n  indented\n1. numbered",
            "only lines that carry on a paragraph join the one before"
        );
    }
}

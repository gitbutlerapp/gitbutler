// Renders the workspace `but panel` serves: the detailed workspace graph plus the uncommitted
// changes. Commit details and file diffs are fetched when a row is opened. Expansion state lives
// here, so the poll never collapses what you opened.

const tree = document.getElementById("tree");
const sub = document.getElementById("sub");
const dot = document.getElementById("dot");
const repoEl = document.getElementById("repo");
const searchEl = document.getElementById("search");
const menuEl = document.getElementById("menu");
const toastEl = document.getElementById("toast");

const POLL_MS = 3000;

// The project this page shows, as `but panel` put it in the URL. Without one the server shows the
// project it was started in.
const PROJECT = new URLSearchParams(location.search).get("project");

/** An API URL for `path`, carrying this page's project. */
function api(path, params = {}) {
	const query = new URLSearchParams(params);
	if (PROJECT) query.set("project", PROJECT);
	return `${path}?${query}`;
}

const open = new Set(); // keys of expanded rows
const loaded = new Map(); // key -> {loading} | {data} | {error}
const urls = new Map(); // key -> where its data was fetched from, for refreshing
let latest = null;
let lastSignature = "";

const esc = (s) =>
	String(s)
		.replace(/&/g, "&amp;")
		.replace(/</g, "&lt;")
		.replace(/>/g, "&gt;")
		.replace(/"/g, "&quot;");

const subject = (msg) => (msg || "").split("\n")[0];

/**
 * Commit bodies are usually hard-wrapped at ~72 columns, which double-wraps into a ragged mess in
 * a narrow panel. Rejoin each paragraph so it reflows, leaving indented blocks and lists alone.
 */
const reflow = (text) =>
	text
		.split(/\n{2,}/)
		.map((para) => {
			const lines = para.split("\n");
			const structured = lines.some(
				(l) => /^\s/.test(l) || /^\s*[-*•]\s/.test(l) || /^\s*\d+[.)]\s/.test(l),
			);
			return structured ? para : lines.join(" ");
		})
		.join("\n\n");

const bodyOf = (msg) => reflow((msg || "").split("\n").slice(1).join("\n").trim());

const words = (s) =>
	String(s || "")
		.replace(/([a-z])([A-Z])/g, "$1 $2")
		.toLowerCase();

const shortDate = (ms) =>
	ms ? new Date(ms).toLocaleDateString(undefined, { month: "short", day: "numeric" }) : "";

const STATUS_CHAR = { Addition: "A", Deletion: "D", Modification: "M", Rename: "R" };
const STATUS_CLASS = {
	Addition: "added",
	Deletion: "removed",
	Modification: "modified",
	Rename: "renamed",
};

// --- data ------------------------------------------------------------------

async function fetchData(url) {
	const response = await fetch(url, { cache: "no-store" });
	const body = await response.json();
	if (!body.ok) throw new Error(body.error);
	return body.data;
}

/** Fetch something for an opened row once, keyed by that row. */
async function load(key, url) {
	urls.set(key, url);
	loaded.set(key, { loading: true });
	paint(true);
	try {
		loaded.set(key, { data: await fetchData(url) });
	} catch (error) {
		loaded.set(key, { error: String(error.message || error) });
	}
	paint(true);
}

/**
 * Re-fetch the open diffs that can still change: uncommitted files, in the main worktree or a
 * linked one. A commit's diffs never change, so they stay as first fetched.
 */
let refreshingDiffs = false;
async function refreshLiveDiffs() {
	if (refreshingDiffs) return;
	refreshingDiffs = true;
	try {
		for (const [key, url] of urls) {
			const entry = loaded.get(key);
			const isLive = url.startsWith("/api/diff") && !new URLSearchParams(url.split("?")[1]).has("commit");
			if (!isLive || !open.has(key) || !entry || entry.loading) continue;
			try {
				loaded.set(key, { data: await fetchData(url) });
			} catch (error) {
				loaded.set(key, { error: String(error.message || error) });
			}
		}
	} finally {
		refreshingDiffs = false;
	}
	// Redraws only if a patch actually changed.
	paint(false);
}

/** Where one file's patch comes from: a commit, a linked worktree, or the main worktree. */
function diffUrl(path, { commit, worktree } = {}) {
	const params = { path };
	if (commit) params.commit = commit;
	if (worktree) params.worktree = worktree;
	return api("/api/diff", params);
}

/**
 * The graph lists rows in display order: a branch row, then its commits, then the next branch.
 * A trailing branch row with no commits is the target the stacks sit on.
 */
function branchesFromRows(stacks) {
	const branches = [];
	for (const stack of stacks) {
		for (const row of stack.rows) {
			if (row.data.type === "Reference") {
				branches.push({ reference: row.data.subject, commits: [] });
			} else if (branches.length) {
				branches[branches.length - 1].commits.push(row.data.subject);
			}
		}
	}
	const last = branches[branches.length - 1];
	const base = last && last.commits.length === 0 ? branches.pop() : null;
	return { branches, base };
}

// --- rendering -------------------------------------------------------------

function renderPatch(key) {
	const entry = loaded.get(key);
	if (!entry || entry.loading) return `<div class="loading">Loading diff…</div>`;
	if (entry.error) return `<div class="err">${esc(entry.error)}</div>`;

	const patch = entry.data;
	if (!patch || patch.type !== "Patch") {
		return `<div class="loading">No textual diff (${esc(words(patch?.type || "empty"))}).</div>`;
	}
	const lines = [];
	for (const hunk of patch.subject.hunks) {
		for (const line of hunk.diff.split("\n")) {
			if (line === "") continue;
			let cls = "";
			if (line.startsWith("@@")) cls = "h";
			else if (line.startsWith("+")) cls = "a";
			else if (line.startsWith("-")) cls = "d";
			lines.push(`<span class="ln ${cls}">${esc(line)}</span>`);
		}
	}
	return `<div class="diff"><pre>${lines.join("")}</pre></div>`;
}

/** File rows for `changes`; opening one fetches its patch from `source`, see `diffUrl()`. */
function renderFiles(changes, source = {}) {
	if (!changes.length) return `<div class="empty">No changes.</div>`;
	return changes
		.map((change) => {
			const key = `file:${source.commit || source.worktree || "uncommitted"}:${change.path}`;
			const isOpen = open.has(key);
			const type = change.status.type;
			const patch = loaded.get(key)?.data?.subject;
			return (
				`<button class="row file ${isOpen ? "open" : ""}" data-key="${esc(key)}" data-url="${esc(diffUrl(change.path, source))}"` +
				` data-path="${esc(change.path)}"${source.worktree ? ` data-worktree="${esc(source.worktree)}"` : ""}${type === "Deletion" ? " data-deleted" : ""}>` +
				`<span class="tw">▶</span>` +
				`<span class="st ${STATUS_CLASS[type] || ""}">${STATUS_CHAR[type] || "?"}</span>` +
				`<span class="grow clip path">&lrm;${esc(change.path)}&lrm;</span>` +
				(patch
					? `<span class="stat"><span class="p">+${patch.linesAdded}</span> <span class="m">−${patch.linesRemoved}</span></span>`
					: "") +
				`</button>` +
				(isOpen ? renderPatch(key) : "")
			);
		})
		.join("");
}

function renderCommit(commit) {
	const key = `commit:${commit.id}`;
	const isOpen = open.has(key);
	const pushed = commit.state?.type !== "LocalOnly";
	const cls = ["commit", pushed ? "pushed" : "", commit.hasConflicts ? "conflicted" : ""].join(" ");

	let html =
		`<button class="row ${cls} ${isOpen ? "open" : ""}" data-key="${esc(key)}" data-url="${esc(api("/api/commit", { id: commit.id }))}">` +
		`<span class="tw">▶</span>` +
		`<span class="grow"><span class="${isOpen ? "wrap" : "clip"}" style="display:block">${esc(subject(commit.message))}</span>` +
		(isOpen
			? `<span class="meta"><span class="id">${esc(commit.id.slice(0, 7))}</span>${esc(commit.author?.name || "")} · ${esc(shortDate(commit.authoredAt))}${
					commit.hasConflicts ? ' · <span style="color:var(--bad)">conflicted</span>' : ""
				}</span>`
			: "") +
		`</span></button>`;

	if (isOpen) {
		const body = bodyOf(commit.message);
		if (body) html += `<div class="body"><pre class="msg">${esc(body)}</pre></div>`;
		const entry = loaded.get(key);
		if (!entry || entry.loading) html += `<div class="loading">Loading files…</div>`;
		else if (entry.error) html += `<div class="err">${esc(entry.error)}</div>`;
		else html += renderFiles(entry.data.changes, { commit: commit.id });
	}
	return html;
}

const CI_MARK = {
	passing: `<span style="color:var(--ok)">✓ CI</span>`,
	pending: `<span style="color:var(--warn)">⏳ CI</span>`,
	failing: `<span style="color:var(--bad)">✗ CI</span>`,
};

function renderBranch({ reference, commits }, reviews) {
	const name = reference.refName.displayName;
	const key = `branch:${reference.refName.fullName}`;
	const isOpen = !open.has(key); // branches start expanded; the key marks "collapsed"
	const bits = [];
	const status = reference.status?.pushStatus;
	if (status) bits.push(esc(words(status)));
	const review = reviews[name];
	if (review) {
		bits.push(
			`<a class="pr" href="${esc(review.url)}" target="_blank" rel="noreferrer">#${review.number}</a>` +
				(review.draft ? " draft" : ""),
		);
		if (review.ci) bits.push(CI_MARK[review.ci]);
	}

	return (
		`<section class="card">` +
		`<button class="row branch ${isOpen ? "open" : ""}" data-key="${esc(key)}">` +
		`<span class="tw">▶</span>` +
		`<span class="grow"><span class="name clip" style="display:block">${esc(name)}</span>` +
		(bits.length ? `<span class="meta">${bits.join(" · ")}</span>` : "") +
		`</span>` +
		`<span class="stat">${commits.length}</span>` +
		`</button>` +
		(isOpen
			? `<div class="commits">${
					commits.length
						? commits.map(renderCommit).join("")
						: `<div class="empty">No commits yet.</div>`
				}</div>`
			: "") +
		`</section>`
	);
}

function renderUncommitted(changes) {
	const key = "uncommitted";
	const isOpen = open.has(key);
	return (
		`<section class="card">` +
		`<button class="row ${isOpen ? "open" : ""}" data-key="${key}">` +
		`<span class="tw">▶</span>` +
		`<span class="grow"><span class="name" style="font-weight:620">Uncommitted</span>` +
		`<span class="meta">${changes.length ? `${changes.length} file${changes.length > 1 ? "s" : ""}` : "no changes"}</span></span>` +
		`</button>` +
		(isOpen && changes.length ? renderFiles(changes) : "") +
		`</section>`
	);
}

/** A linked worktree: its branch, where it lives, and its uncommitted changes. */
function renderWorktree({ worktree, changes = [], error, archived }) {
	const key = `worktree:${worktree.name}`;
	// An archived worktree has no readable changes, so there is nothing to open.
	const isOpen = !archived && open.has(key);
	const branch = worktree.refName ? worktree.refName.replace(/^refs\/heads\//, "") : null;
	const state = archived
		? "archived"
		: error
		? `<span style="color:var(--bad)">unreadable</span>`
		: changes.length
			? `<span style="color:var(--warn)">${changes.length} changed</span>`
			: "clean";

	return (
		`<section class="card"${archived ? ' style="opacity:.6"' : ""}>` +
		`<button class="row branch ${isOpen ? "open" : ""}"${archived ? "" : ` data-key="${esc(key)}"`}>` +
		`<span class="tw">${archived ? "" : "▶"}</span>` +
		`<span class="grow"><span class="name clip" style="display:block">${esc(branch || `${worktree.name} (detached)`)}</span>` +
		`<span class="meta">${state}</span>` +
		`<span class="meta"><span class="clip path" style="display:block">&lrm;${esc(worktree.path)}&lrm;</span></span>` +
		`</span></button>` +
		(isOpen
			? error
				? `<div class="err">${esc(error)}</div>`
				: renderFiles(changes, { worktree: worktree.name })
			: "") +
		`</section>`
	);
}

function render({ workspace, changes, worktrees, reviews }) {
	const { branches, base } = branchesFromRows(workspace.stacks);
	if (query) {
		tree.innerHTML = renderSearch(branches, changes, worktrees);
		return;
	}
	const parts = [
		renderUncommitted(changes),
		...branches.map((branch) => renderBranch(branch, reviews)),
	];
	if (base) {
		parts.push(`<div class="base"><span class="clip">${esc(base.reference.refName.displayName)}</span></div>`);
	}
	// Only listed with the `worktreeManipulation` feature flag on.
	if (worktrees.length) {
		parts.push(`<div class="section">Worktrees <span class="stat">${worktrees.length}</span></div>`);
		parts.push(...worktrees.map(renderWorktree));
	}
	tree.innerHTML = parts.join("");
}

function paint(force) {
	if (!latest) return;
	const signature =
		JSON.stringify(latest) +
		[...open].sort().join("|") +
		JSON.stringify([...loaded]) +
		query +
		(commitFiles ? commitFiles.key : "");
	if (!force && signature === lastSignature) return;
	lastSignature = signature;
	const y = window.scrollY;
	render(latest);
	window.scrollTo(0, y);
}

async function tick() {
	try {
		const data = await fetchData(api("/api/workspace"));
		dot.className = "dot";
		showProject(data);
		document.title = `${data.repo} — workspace`;
		sub.textContent = new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
		latest = {
			project: data.project,
			workspace: data.workspace,
			changes: data.changes,
			worktrees: data.worktrees,
			reviews: data.reviews,
		};
		paint(false);
		refreshLiveDiffs();
		if (query) loadCommitFiles();
	} catch (error) {
		dot.className = "dot bad";
		if (!latest) tree.innerHTML = `<div class="err">${esc(error.message || error)}</div>`;
	}
}

// --- file search -----------------------------------------------------------

let query = "";
let commitFiles = null; // { key, files: { [commitId]: changes } } for the workspace's commits
let commitFilesLoading = null;

const commitIdsOf = (workspace) =>
	workspace.stacks
		.flatMap((stack) => stack.rows)
		.filter((row) => row.data.type === "Commit")
		.map((row) => row.data.subject.id);

/** Fetch the workspace commits' files when the set of commits changed since the last fetch. */
async function loadCommitFiles() {
	if (!latest) return;
	const key = commitIdsOf(latest.workspace).join(",");
	if (commitFiles?.key === key || commitFilesLoading === key) return;
	commitFilesLoading = key;
	try {
		commitFiles = { key, files: await fetchData(api("/api/commit-files")) };
	} catch (error) {
		commitFiles = { key, error: String(error.message || error) };
	}
	commitFilesLoading = null;
	paint(true);
}

/** Every term must appear in the path, in any order and case. */
function matches(path) {
	const lower = path.toLowerCase();
	return query
		.toLowerCase()
		.split(/\s+/)
		.filter(Boolean)
		.every((term) => lower.includes(term));
}

function renderSearch(branches, changes, worktrees) {
	const groups = [];
	const addGroup = (title, detail, groupChanges, source) => {
		const found = groupChanges.filter((change) => matches(change.path));
		if (!found.length) return;
		groups.push(
			`<section class="card"><div class="group"><span class="name clip">${esc(title)}</span>` +
				(detail ? `<span class="meta clip">${esc(detail)}</span>` : "") +
				`</div>${renderFiles(found, source)}</section>`,
		);
	};

	addGroup("Uncommitted", "", changes, {});
	for (const { reference, commits } of branches) {
		for (const commit of commits) {
			const files = commitFiles?.files?.[commit.id];
			if (files) {
				addGroup(reference.refName.displayName, subject(commit.message), files, {
					commit: commit.id,
				});
			}
		}
	}
	for (const { worktree, changes: worktreeChanges = [] } of worktrees) {
		addGroup(`${worktree.name} worktree`, "uncommitted", worktreeChanges, {
			worktree: worktree.name,
		});
	}

	let status = "";
	if (commitFiles?.error) status = `<div class="err">${esc(commitFiles.error)}</div>`;
	else if (commitFiles?.key !== commitIdsOf(latest.workspace).join(","))
		status = `<div class="loading">Searching commits…</div>`;
	return status + (groups.length ? groups.join("") : status ? "" : `<div class="empty">No files match.</div>`);
}

searchEl.addEventListener("input", () => {
	query = searchEl.value.trim();
	if (query) loadCommitFiles();
	paint(true);
});

addEventListener("keydown", (event) => {
	if (event.key === "/" && document.activeElement !== searchEl) {
		event.preventDefault();
		searchEl.focus();
	} else if (event.key === "Escape") {
		if (!menuEl.hidden) closeMenu();
		else if (document.activeElement === searchEl && searchEl.value) {
			searchEl.value = "";
			searchEl.dispatchEvent(new Event("input"));
		}
	}
});

// --- open with ---------------------------------------------------------------

const programsByExtension = new Map();

/** The programs for `path`, which depend only on its extension. */
async function programsFor(path) {
	const extension = path.includes(".") ? path.slice(path.lastIndexOf(".")) : "";
	if (!programsByExtension.has(extension)) {
		programsByExtension.set(extension, await fetchData(api("/api/programs", { path })));
	}
	return programsByExtension.get(extension);
}

function closeMenu() {
	menuEl.hidden = true;
	menuEl.innerHTML = "";
}

/** Show the menu at the pointer, kept inside the window. */
function placeMenu(x, y) {
	menuEl.hidden = false;
	const { width, height } = menuEl.getBoundingClientRect();
	menuEl.style.left = `${Math.max(4, Math.min(x, innerWidth - width - 4))}px`;
	menuEl.style.top = `${Math.max(4, Math.min(y, innerHeight - height - 4))}px`;
}

function toast(message, bad) {
	toastEl.textContent = message;
	toastEl.className = `toast${bad ? " bad" : ""}`;
	toastEl.hidden = false;
	clearTimeout(toast.timer);
	toast.timer = setTimeout(() => (toastEl.hidden = true), bad ? 5000 : 2000);
}

/** The absolute path of `path` in the checkout it belongs to: a linked worktree or the project. */
function fullPath(path, worktree) {
	const root = worktree
		? latest.worktrees.find((entry) => entry.worktree.name === worktree)?.worktree.path
		: latest.project;
	return root ? `${root.replace(/\/$/, "")}/${path}` : path;
}

const menuItem = (label, data) =>
	`<button class="menu-item" ${Object.entries(data)
		.map(([key, value]) => `data-${key}="${esc(value)}"`)
		.join(" ")}>${esc(label)}</button>`;

tree.addEventListener("contextmenu", async (event) => {
	const row = event.target.closest(".row.file[data-path]");
	if (!row) return;
	event.preventDefault();
	const { path, worktree } = row.dataset;
	menuEl.dataset.path = path;
	menuEl.dataset.worktree = worktree || "";

	const header =
		`<div class="menu-title clip">&lrm;${esc(path)}&lrm;</div>` +
		`<div class="menu-label">Copy</div>` +
		menuItem("File name", { copy: path.slice(path.lastIndexOf("/") + 1), what: "File name" }) +
		menuItem("Relative path", { copy: path, what: "Relative path" }) +
		menuItem("Full path", { copy: fullPath(path, worktree), what: "Full path" }) +
		`<div class="menu-label">Open with</div>`;
	const show = (openWith) => {
		menuEl.innerHTML = header + openWith;
		placeMenu(event.clientX, event.clientY);
	};

	if ("deleted" in row.dataset) {
		show(`<div class="menu-note">Deleted, so there is nothing to open.</div>`);
		return;
	}
	show(`<div class="menu-note">Loading…</div>`);
	try {
		const programs = await programsFor(path);
		if (menuEl.hidden || menuEl.dataset.path !== path) return;
		show(
			programs.length
				? programs.map((program) => menuItem(program.name, { program: program.id, name: program.name })).join("")
				: `<div class="menu-note">No programs found.</div>`,
		);
	} catch (error) {
		show(`<div class="menu-note err-text">${esc(error.message || error)}</div>`);
	}
});

/** Copy `text`, falling back to a selection where the clipboard API isn't allowed. */
async function copyText(text) {
	try {
		await navigator.clipboard.writeText(text);
	} catch {
		const field = document.createElement("textarea");
		field.value = text;
		document.body.append(field);
		field.select();
		const copied = document.execCommand("copy");
		field.remove();
		if (!copied) throw new Error("The browser didn't allow copying");
	}
}

menuEl.addEventListener("click", async (event) => {
	const item = event.target.closest(".menu-item");
	if (!item) return;
	const { path, worktree } = menuEl.dataset;
	closeMenu();

	if ("copy" in item.dataset) {
		try {
			await copyText(item.dataset.copy);
			toast(`${item.dataset.what} copied`);
		} catch (error) {
			toast(String(error.message || error), true);
		}
		return;
	}

	const params = { path, program: item.dataset.program };
	if (worktree) params.worktree = worktree;
	try {
		const response = await fetch(api("/api/open", params), { method: "POST" });
		const body = await response.json().catch(() => ({ ok: false, error: response.statusText }));
		if (!body.ok) throw new Error(body.error);
		toast(`Opened in ${item.dataset.name}`);
	} catch (error) {
		toast(String(error.message || error), true);
	}
});

addEventListener("mousedown", (event) => {
	if (!menuEl.hidden && !menuEl.contains(event.target)) closeMenu();
});
addEventListener("scroll", () => menuEl.hidden || closeMenu(), true);
addEventListener("resize", () => menuEl.hidden || closeMenu());

// --- project switcher ------------------------------------------------------

let projectList = null;
let shownProject = null;

/** Fill the header's switcher once, marking the project this page shows. */
async function showProject({ repo, project }) {
	if (projectList === null) {
		try {
			projectList = await fetchData(api("/api/projects"));
		} catch {
			projectList = [];
		}
	}
	if (shownProject === project && repoEl.options.length) return;
	shownProject = project;
	const entries = projectList.some((entry) => entry.path === project)
		? projectList
		: [{ name: repo, path: project }, ...projectList];
	repoEl.innerHTML = entries
		.map(
			(entry) =>
				`<option value="${esc(entry.path)}"${entry.path === project ? " selected" : ""}>${esc(entry.name)}</option>`,
		)
		.join("");
}

repoEl.addEventListener("change", () => {
	// A full navigation, so the address, reloads and history all name the new project.
	// Keep slashes readable, the way `but panel` prints the URL.
	location.search = `project=${encodeURIComponent(repoEl.value).replaceAll("%2F", "/")}`;
});

tree.addEventListener("click", (event) => {
	const row = event.target.closest("[data-key]");
	// A review link opens the forge; it isn't a click on the row.
	if (!row || event.target.closest("a")) return;
	const { key, url } = row.dataset;

	if (open.has(key)) open.delete(key);
	else open.add(key);

	// Rows with something to fetch fetch it the first time they open.
	if (url && open.has(key) && !loaded.has(key)) {
		load(key, url);
		return;
	}
	paint(true);
});

// Refetch what is open; closed rows fetch again when they next open.
document.getElementById("refresh").addEventListener("click", () => {
	for (const key of loaded.keys()) {
		if (open.has(key)) load(key, urls.get(key));
		else loaded.delete(key);
	}
	tick();
});

tick();
setInterval(tick, POLL_MS);
addEventListener("focus", tick);

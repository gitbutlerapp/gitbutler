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

/** An API URL for `path`, carrying this page's project. `params` may repeat a key as pairs. */
function api(path, params = {}) {
	const query = new URLSearchParams(params);
	if (PROJECT) query.set("project", PROJECT);
	return `${path}?${query}`;
}

// The web app manifest for this project, so "Install" in a Chromium browser makes the panel a
// windowed app of its own for this project.
if (PROJECT) document.getElementById("manifest").href = api("/manifest.webmanifest");

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
 * The graph lists rows in display order: a branch row, then its commits, then the branch below it.
 * Branches stacked on each other are drawn as one unbroken line: the branch below continues in the
 * same column, right after a commit that neither links its line into another column nor ends it.
 * Anything else starts an independent stack, as do the rows of the next graph stack. A trailing
 * branch row with no commits is the target the stacks sit on. Returns the branches per stack, all
 * of them flat, and that target.
 */
function branchesFromRows(stacks) {
	const grouped = [];
	for (const stack of stacks) {
		let column = null;
		let ended = true;
		for (const row of stack.rows) {
			if (row.data.type === "Reference") {
				const rowColumn = row.nodeLine.indexOf("node");
				if (ended || rowColumn !== column) grouped.push([]);
				column = rowColumn;
				grouped[grouped.length - 1].push({ reference: row.data.subject, commits: [] });
			} else if (grouped.length) {
				const branches = grouped[grouped.length - 1];
				branches[branches.length - 1].commits.push(row.data.subject);
			}
			ended = Boolean(row.linkLine || row.termLine);
		}
	}
	const lastStack = grouped[grouped.length - 1];
	const last = lastStack?.[lastStack.length - 1];
	const base = last && last.commits.length === 0 ? lastStack.pop() : null;
	return { stacks: grouped.filter((branches) => branches.length), branches: grouped.flat(), base };
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
		`<button class="row ${cls} ${isOpen ? "open" : ""}" data-key="${esc(key)}" data-url="${esc(api("/api/commit", { id: commit.id }))}" data-commit="${esc(commit.id)}">` +
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

function renderBranch({ reference, commits }, forge) {
	const name = reference.refName.displayName;
	const key = `branch:${reference.refName.fullName}`;
	const isOpen = !open.has(key); // branches start expanded; the key marks "collapsed"
	const bits = [];
	const status = reference.status?.pushStatus;
	if (status) bits.push(esc(words(status)));
	const review = forge?.branches[name]?.review;
	if (review) {
		bits.push(
			`<a class="pr" href="${esc(review.url)}" target="_blank" rel="noreferrer">#${review.number}</a>` +
				(review.draft ? " draft" : ""),
		);
		if (review.ci) bits.push(CI_MARK[review.ci]);
	}

	return (
		`<section class="card">` +
		`<button class="row branch ${isOpen ? "open" : ""}" data-key="${esc(key)}" data-branch="${esc(reference.refName.fullName)}">` +
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

function render({ workspace, behind, changes, worktrees, forge }) {
	const { stacks, branches, base } = branchesFromRows(workspace.stacks);
	if (query) {
		tree.innerHTML = renderSearch(branches, changes, worktrees);
		return;
	}
	const parts = [renderUncommitted(changes)];
	for (const group of stacks) {
		const cards = group.map((branch) => renderBranch(branch, forge)).join("");
		parts.push(
			group.length > 1
				? `<div class="stack"><div class="stack-label">Stack · ${group.length} branches</div>${cards}</div>`
				: cards,
		);
	}
	if (base) {
		// As of the last fetch; the Fetch button updates it.
		const status =
			behind > 0
				? `<span class="behind" title="Commits on the remote that the workspace doesn't have yet">⇣ ${behind} behind</span>`
				: behind === 0
					? `<span>up to date</span>`
					: "";
		parts.push(
			`<div class="base"><span class="clip">${esc(base.reference.refName.displayName)}</span>${status ? " · " + status : ""}</div>`,
		);
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
			behind: data.behind,
			changes: data.changes,
			worktrees: data.worktrees,
			forge: data.forge,
			autoFetchMinutes: data.autoFetchMinutes,
		};
		paint(false);
		showUpstream();
		refreshLiveDiffs();
		if (query) loadCommitFiles();
	} catch (error) {
		dot.className = "dot bad";
		if (latest) return;
		// A refused connection, rather than an answer, means no `but panel` is serving. The poll
		// keeps trying, so starting one is all it takes.
		tree.innerHTML =
			error instanceof TypeError
				? `<div class="down"><div class="down-title">The panel isn't running</div>` +
					`<p>Start it from any GitButler project. This page connects on its own once it's up.</p>` +
					`<pre class="cmd">but panel --no-open</pre><button class="ghost" id="copy-cmd">Copy command</button></div>`
				: `<div class="err">${esc(error.message || error)}</div>`;
	}
}

// Keeps the page itself at hand for when the server is down; see sw.js.
if ("serviceWorker" in navigator) {
	navigator.serviceWorker
		.register("/sw.js")
		.then((registration) => registration.update())
		.catch((error) => console.warn("The page won't be kept for offline use:", error));
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

// --- installing --------------------------------------------------------------

// A Chromium browser offers this once the page qualifies as an installable app, and the project
// menu then offers to install. It never comes in a window that already is the app, nor in
// browsers without app installs, so the item stays out of the way there.
let installPrompt = null;
addEventListener("beforeinstallprompt", (event) => {
	event.preventDefault();
	installPrompt = event;
});
addEventListener("appinstalled", () => {
	installPrompt = null;
	toast("Installed; find it with your other apps");
});

// --- context menus -----------------------------------------------------------

const programsByExtension = new Map();

/** The programs for `path`, which depend only on its extension. */
async function programsFor(path) {
	const extension = path.includes(".") ? path.slice(path.lastIndexOf(".")) : "";
	if (!programsByExtension.has(extension)) {
		programsByExtension.set(extension, await fetchData(api("/api/programs", { path })));
	}
	return programsByExtension.get(extension);
}

let editors = null;

/** The editors that open a commit's files together. */
async function editorsList() {
	editors ??= await fetchData(api("/api/programs"));
	return editors;
}

let openers = null;

/** What can open the project's folder, each with the query that asks for it. */
async function folderOpeners() {
	openers ??= await fetchData(api("/api/folder-openers"));
	return openers;
}

/** Ask the server to do something, and return what it reports. */
async function post(path, params) {
	const response = await fetch(api(path, params), { method: "POST" });
	const body = await response.json().catch(() => ({ ok: false, error: response.statusText }));
	if (!body.ok) throw new Error(body.error);
	return body.data;
}

// Counts the menus shown, so a fetch for one menu can't fill in a later one or reopen a closed one.
let menuSerial = 0;

function closeMenu() {
	menuSerial++;
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
const menuLabel = (text) => `<div class="menu-label">${esc(text)}</div>`;
const menuNote = (text, bad) => `<div class="menu-note${bad ? " err-text" : ""}">${esc(text)}</div>`;

/** The "open with" items for `programs`, or `none` when there aren't any. */
const programItems = (programs, none) =>
	programs.length
		? programs.map((program) => menuItem(program.name, { program: program.id, name: program.name })).join("")
		: menuNote(none);

/** Show `html` as menu number `serial` at the pointer, unless that menu has been replaced. */
function showMenu(serial, event, html) {
	if (serial !== menuSerial) return;
	menuEl.innerHTML = html;
	placeMenu(event.clientX, event.clientY);
}

/** The commit with `id` in the workspace graph. */
function commitById(id) {
	for (const stack of latest.workspace.stacks) {
		for (const row of stack.rows) {
			if (row.data.type === "Commit" && row.data.subject.id === id) return row.data.subject;
		}
	}
	return null;
}

/** A commit's details, as its row shows them, fetched now if the row was never opened. */
async function commitDetails(id) {
	const key = `commit:${id}`;
	const entry = loaded.get(key);
	if (entry?.data) return entry.data;
	const url = api("/api/commit", { id });
	const data = await fetchData(url);
	urls.set(key, url);
	loaded.set(key, { data });
	return data;
}

async function fileMenu(event, serial, row) {
	const { path, worktree } = row.dataset;
	menuEl.dataset.paths = JSON.stringify([path]);
	menuEl.dataset.worktree = worktree || "";

	const header =
		`<div class="menu-title path clip">&lrm;${esc(path)}&lrm;</div>` +
		menuLabel("Copy") +
		menuItem("File name", { copy: path.slice(path.lastIndexOf("/") + 1), what: "File name" }) +
		menuItem("Relative path", { copy: path, what: "Relative path" }) +
		menuItem("Full path", { copy: fullPath(path, worktree), what: "Full path" }) +
		menuLabel("Open with");
	if ("deleted" in row.dataset) {
		showMenu(serial, event, header + menuNote("Deleted, so there is nothing to open."));
		return;
	}
	showMenu(serial, event, header + menuNote("Loading…"));
	try {
		showMenu(serial, event, header + programItems(await programsFor(path), "No programs found."));
	} catch (error) {
		showMenu(serial, event, header + menuNote(error.message || error, true));
	}
}

async function commitMenu(event, serial, commit) {
	const forge = latest.forge;
	const pushed = commit.state?.type !== "LocalOnly";
	let header =
		`<div class="menu-title clip">${esc(subject(commit.message))}</div>` +
		menuLabel("Copy") +
		menuItem("Short ID", { copy: commit.id.slice(0, 7), what: "Commit ID" }) +
		menuItem("Full ID", { copy: commit.id, what: "Commit ID" }) +
		menuItem("Change ID", { copy: commit.changeId, what: "Change ID" }) +
		menuItem("Subject", { copy: subject(commit.message), what: "Subject" }) +
		menuItem("Full message", { copy: commit.message.trimEnd(), what: "Message" });
	if (forge) {
		header +=
			menuLabel(forge.name) +
			(pushed
				? menuItem("Open commit", { href: forge.commitUrl + commit.id })
				: menuNote(`Not pushed yet, so it isn't on ${forge.name}.`));
	}
	header += menuLabel("Open all changed files with");
	showMenu(serial, event, header + menuNote("Loading…"));
	try {
		const [details, programs] = await Promise.all([commitDetails(commit.id), editorsList()]);
		if (serial !== menuSerial) return;
		// A deleted file has no working copy to open; a renamed one is opened at its new path.
		const paths = details.changes.filter((change) => change.status.type !== "Deletion").map((change) => change.path);
		menuEl.dataset.paths = JSON.stringify(paths);
		showMenu(
			serial,
			event,
			header +
				(paths.length
					? programItems(programs, "No editors found.")
					: menuNote("Only deletions, so there is nothing to open.")),
		);
	} catch (error) {
		showMenu(serial, event, header + menuNote(error.message || error, true));
	}
}

function branchMenu(event, serial, { reference, commits }) {
	const name = reference.refName.displayName;
	const forge = latest.forge;
	const onForge = forge?.branches[name];
	// Oldest first, the order a review reads them in.
	const commitList = commits
		.map((commit) => `- ${subject(commit.message)}`)
		.reverse()
		.join("\n");
	// A push takes the branches below this one along, so on a stack's top branch it pushes the stack.
	const pushStatus = reference.status?.pushStatus;
	const force = pushStatus === "unpushedCommitsRequiringForce";
	const pushable = force || pushStatus === "unpushedCommits" || pushStatus === "completelyUnpushed";
	let html = `<div class="menu-title clip">${esc(name)}</div>`;
	if (pushable) {
		html += menuItem(force ? "Force push" : "Push", {
			push: reference.refName.fullName,
			name,
			...(force ? { force: "1" } : {}),
		});
	}
	html +=
		menuLabel("Copy") +
		menuItem("Branch name", { copy: name, what: "Branch name" }) +
		(commits.length ? menuItem("Commit list", { copy: commitList, what: "Commit list" }) : "");
	if (forge) {
		const unit = forge.unit.abbr;
		const review = onForge?.review;
		html += menuLabel(forge.name);
		if (review) {
			html +=
				menuItem(`Open ${unit} #${review.number}`, { href: review.url }) +
				menuItem(`Copy ${unit} link`, { copy: review.url, what: `${unit} link` });
		}
		if (pushStatus === "completelyUnpushed") {
			html += menuNote(`Not pushed yet, so it isn't on ${forge.name}.`);
		} else if (onForge?.url) {
			html += menuItem("Open branch", { href: onForge.url });
		}
	}
	showMenu(serial, event, html);
}

tree.addEventListener("contextmenu", (event) => {
	const row = event.target.closest(".row[data-path], .row[data-commit], .row[data-branch]");
	if (!row) return;
	event.preventDefault();
	const serial = ++menuSerial;
	menuEl.dataset.paths = "";
	menuEl.dataset.worktree = "";
	const { path, commit, branch } = row.dataset;
	if (path !== undefined) {
		fileMenu(event, serial, row);
	} else if (commit) {
		const found = commitById(commit);
		if (found) commitMenu(event, serial, found);
	} else {
		const found = branchesFromRows(latest.workspace.stacks).branches.find(
			(entry) => entry.reference.refName.fullName === branch,
		);
		if (found) branchMenu(event, serial, found);
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
	const { paths, worktree } = menuEl.dataset;
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
	if ("href" in item.dataset) {
		window.open(item.dataset.href, "_blank", "noopener");
		return;
	}

	try {
		if ("with" in item.dataset) {
			await post("/api/open-folder", item.dataset.with);
			toast(`Opened in ${item.dataset.name}`);
			return;
		}
		if ("install" in item.dataset) {
			const prompt = installPrompt;
			installPrompt = null;
			await prompt.prompt();
			return;
		}
		if ("autofetch" in item.dataset) {
			await post("/api/settings", { autoFetchMinutes: item.dataset.autofetch });
			toast(`Auto-fetch: ${item.dataset.label.toLowerCase()}`);
			tick();
			return;
		}
		if ("push" in item.dataset) {
			toast(`Pushing ${item.dataset.name}…`);
			const params = { branch: item.dataset.push };
			if ("force" in item.dataset) params.force = "1";
			const { pushed } = await post("/api/push", params);
			toast(pushed.length ? `Pushed ${pushed.join(", ")}` : "Nothing to push");
			tick();
			return;
		}
		const params = JSON.parse(paths || "[]").map((path) => ["path", path]);
		params.push(["program", item.dataset.program]);
		if (worktree) params.push(["worktree", worktree]);
		const { opened } = await post("/api/open", params);
		toast(opened === 1 ? `Opened in ${item.dataset.name}` : `Opened ${opened} files in ${item.dataset.name}`);
	} catch (error) {
		toast(String(error.message || error), true);
	}
});

// What the app offers for its auto-fetch frequency, in minutes; a negative value turns it off.
const AUTO_FETCH = [
	[1, "Every minute"],
	[5, "Every 5 minutes"],
	[10, "Every 10 minutes"],
	[15, "Every 15 minutes"],
	[-1, "Off"],
];

/** The project menu, below its button: the path, the forge page, and what opens the folder. */
document.getElementById("more").addEventListener("click", async (event) => {
	if (!latest) return;
	const serial = ++menuSerial;
	menuEl.dataset.paths = "";
	menuEl.dataset.worktree = "";
	const rect = event.currentTarget.getBoundingClientRect();
	const at = { clientX: rect.left, clientY: rect.bottom + 4 };
	const forge = latest.forge;

	let header =
		`<div class="menu-title path clip">&lrm;${esc(latest.project)}&lrm;</div>` +
		menuLabel("Copy") +
		menuItem("Project path", { copy: latest.project, what: "Project path" });
	if (forge) header += menuLabel(forge.name) + menuItem("Open repository", { href: forge.url });
	if (installPrompt) header += menuLabel("This page") + menuItem("Install as an app", { install: "" });
	// The app's own setting, so changing it here changes it there too.
	header +=
		menuLabel("Auto-fetch") +
		AUTO_FETCH.map(([minutes, label]) =>
			menuItem(`${minutes === latest.autoFetchMinutes ? "✓ " : " "}${label}`, { autofetch: minutes, label }),
		).join("");
	header += menuLabel("Open folder with");
	showMenu(serial, at, header + menuNote("Loading…"));
	try {
		const items = (await folderOpeners()).map((opener) =>
			menuItem(opener.name, { with: opener.with, name: opener.name }),
		);
		showMenu(serial, at, header + items.join(""));
	} catch (error) {
		showMenu(serial, at, header + menuNote(error.message || error, true));
	}
});

const PULL_WORD = {
	updatable: "rebase",
	integrated: "merged upstream, will be removed",
	conflicted_rebasable: "will conflict",
};

const upstreamEl = document.getElementById("upstream");
let pulling = false;
let pullPreview = null; // what a pull would do, shown in the notice until confirmed or cancelled

/** The target's name as the base row shows it. */
const baseName = () => branchesFromRows(latest.workspace.stacks).base?.reference.refName.displayName || "the target";

const plural = (count, noun) => `${count} ${noun}${count === 1 ? "" : noun === "branch" ? "es" : "s"}`;

/** The header's notice that the target moved on, with the Pull button, while the workspace is behind. */
function showUpstream() {
	// A preview being read or a pull in progress keeps the notice as it is until it's done.
	if (pulling || pullPreview) return;
	const behind = latest?.behind;
	upstreamEl.hidden = !(behind > 0);
	upstreamEl.classList.remove("open");
	// Redrawn only when it would change, so a poll never replaces the button mid-click.
	const state = behind > 0 ? `${behind}:${baseName()}` : "";
	if (state && upstreamEl.dataset.state !== state) {
		upstreamEl.innerHTML =
			`<span>⇣ ${plural(behind, "new commit")} on ${esc(baseName())}</span>` + `<button class="pull">Pull</button>`;
	}
	upstreamEl.dataset.state = state;
}

/** The notice grown into a question: what rebasing each branch would do, and the buttons to answer. */
function showPullPreview(preview) {
	const lines = preview.branches
		.map(
			(branch) =>
				`<div class="line"><span class="clip">${esc(branch.name)}</span>` +
				`<span class="word">${esc(PULL_WORD[branch.status] || branch.status)}</span></div>`,
		)
		.join("");
	const refused = preview.worktreeConflicts.length
		? `<div class="refused">Uncommitted changes in ${plural(preview.worktreeConflicts.length, "file")} would conflict, so the pull will be refused.</div>`
		: "";
	upstreamEl.classList.add("open");
	upstreamEl.innerHTML =
		`<div class="title">Rebase ${plural(preview.branches.length, "branch")} onto ${esc(baseName())}?</div>` +
		lines +
		refused +
		`<div class="actions"><span class="hint">but undo reverts it</span>` +
		`<button class="cancel">Cancel</button><button class="pull confirm">Pull</button></div>`;
}

/**
 * Pull: first show what rebasing every stack onto the target would do, and once confirmed, do it.
 * The rebase runs on the server without stopping; commits that conflict are marked, not left half
 * done.
 */
upstreamEl.addEventListener("click", async (event) => {
	const button = event.target.closest("button");
	if (!button || pulling) return;

	if (button.classList.contains("cancel")) {
		pullPreview = null;
		showUpstream();
		return;
	}
	if (!button.classList.contains("confirm")) {
		button.disabled = true;
		button.textContent = "Checking…";
		try {
			pullPreview = await post("/api/pull", { check: "1" });
			showPullPreview(pullPreview);
		} catch (error) {
			toast(String(error.message || error), true);
			button.disabled = false;
			button.textContent = "Pull";
		}
		return;
	}

	pulling = true;
	button.disabled = true;
	button.textContent = "Pulling…";
	try {
		const result = await post("/api/pull");
		const conflicted = result.branches
			.filter((branch) => branch.status === "conflicted_rebasable")
			.map((branch) => branch.name);
		toast(
			conflicted.length
				? `Pulled with conflicts in ${conflicted.join(", ")}: resolve them with but resolve, or but undo`
				: `Pulled: ${plural(result.branches.length, "branch")} rebased`,
			conflicted.length > 0,
		);
	} catch (error) {
		toast(String(error.message || error), true);
	} finally {
		pulling = false;
		pullPreview = null;
		// Redraws the notice, or hides it once the workspace has caught up.
		tick();
	}
});

/** Fetch from the remotes, then show what the target gained. */
const fetchEl = document.getElementById("fetch");
fetchEl.addEventListener("click", async () => {
	fetchEl.disabled = true;
	fetchEl.textContent = "Fetching…";
	try {
		await post("/api/fetch");
		await tick();
		const behind = latest?.behind;
		toast(behind > 0 ? `Fetched: ${behind} new commit${behind > 1 ? "s" : ""} on the target` : "Fetched, up to date");
	} catch (error) {
		toast(String(error.message || error), true);
	} finally {
		fetchEl.disabled = false;
		fetchEl.textContent = "⇣ Fetch";
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
	repoEl.innerHTML =
		entries
			.map(
				(entry) =>
					`<option value="${esc(entry.path)}"${entry.path === project ? " selected" : ""}>${esc(entry.name)}</option>`,
			)
			.join("") + `<option disabled>──────</option><option value="${ADD_PROJECT}">Add project…</option>`;
}

// A relative path, which no project root can be.
const ADD_PROJECT = "+";
const addForm = document.getElementById("add-project");
const addPath = document.getElementById("add-path");

/** Show the page for the project at `path`: a full navigation, so the address, reloads and history
 * all name it. Slashes stay readable, the way `but panel` prints the URL. */
function showProjectAt(path) {
	location.search = `project=${encodeURIComponent(path).replaceAll("%2F", "/")}`;
}

repoEl.addEventListener("change", () => {
	if (repoEl.value !== ADD_PROJECT) {
		showProjectAt(repoEl.value);
		return;
	}
	repoEl.value = shownProject;
	addForm.hidden = false;
	addPath.focus();
});

document.getElementById("add-cancel").addEventListener("click", () => {
	addForm.hidden = true;
	addPath.value = "";
});

/** Add the repository typed in, then show it. It stays listed after the server stops when
 * GitButler could remember it as a project. */
addForm.addEventListener("submit", async (event) => {
	event.preventDefault();
	const path = addPath.value.trim();
	if (!path) return;
	const button = addForm.querySelector("button[type=submit]");
	button.disabled = true;
	try {
		const added = await post("/api/projects", { path });
		showProjectAt(added.path);
	} catch (error) {
		toast(String(error.message || error), true);
		button.disabled = false;
	}
});

tree.addEventListener("click", (event) => {
	if (event.target.closest("#copy-cmd")) {
		copyText(tree.querySelector(".down .cmd").textContent).then(
			() => toast("Command copied"),
			(error) => toast(String(error.message || error), true),
		);
		return;
	}
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

// Poll only while the page can be seen: a hidden tab costs the server nothing, and catches up the
// moment it's shown again.
let poll = null;
function watchVisibility() {
	clearInterval(poll);
	poll = null;
	if (document.visibilityState !== "visible") return;
	tick();
	poll = setInterval(tick, POLL_MS);
}
document.addEventListener("visibilitychange", watchVisibility);
addEventListener("focus", () => document.visibilityState === "visible" && tick());
watchVisibility();

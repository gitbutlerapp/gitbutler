// Renders the workspace `but panel` serves: the detailed workspace graph plus the uncommitted
// changes. Commit details and file diffs are fetched when a row is opened. Expansion state lives
// here, so the poll never collapses what you opened.

const tree = document.getElementById("tree");
const sub = document.getElementById("sub");
const dot = document.getElementById("dot");
const repoEl = document.getElementById("repo");

const POLL_MS = 3000;

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

/** Where one file's patch comes from: a commit, a linked worktree, or the main worktree. */
function diffUrl(path, { commit, worktree } = {}) {
	const params = new URLSearchParams({ path });
	if (commit) params.set("commit", commit);
	if (worktree) params.set("worktree", worktree);
	return `/api/diff?${params}`;
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
				`<button class="row file ${isOpen ? "open" : ""}" data-key="${esc(key)}" data-url="${esc(diffUrl(change.path, source))}">` +
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
		`<button class="row ${cls} ${isOpen ? "open" : ""}" data-key="${esc(key)}" data-url="/api/commit?id=${esc(commit.id)}">` +
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
		JSON.stringify(latest) + [...open].sort().join("|") + JSON.stringify([...loaded]);
	if (!force && signature === lastSignature) return;
	lastSignature = signature;
	const y = window.scrollY;
	render(latest);
	window.scrollTo(0, y);
}

async function tick() {
	try {
		const data = await fetchData("/api/workspace");
		dot.className = "dot";
		repoEl.textContent = data.repo;
		document.title = `${data.repo} — workspace`;
		sub.textContent = new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
		latest = {
			workspace: data.workspace,
			changes: data.changes,
			worktrees: data.worktrees,
			reviews: data.reviews,
		};
		paint(false);
	} catch (error) {
		dot.className = "dot bad";
		if (!latest) tree.innerHTML = `<div class="err">${esc(error.message || error)}</div>`;
	}
}

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

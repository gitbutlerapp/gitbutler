import {
	type McpUiToolResultNotification,
	useApp,
	useHostStyles,
} from "@modelcontextprotocol/ext-apps/react";
import { useRef, useState } from "react";
import type {
	Commit,
	CommitDetails,
	DetailedGraphReference,
	DetailedGraphRow,
	DetailedGraphStack,
	DetailedGraphWorkspace,
	PushStatus,
} from "@gitbutler/but-sdk";
import type { App } from "@modelcontextprotocol/ext-apps";

type WorkspaceView = {
	version: number;
	repository: {
		name: string;
		path: string;
	};
	summary: {
		stacks: number;
		branches: number;
		commits: number;
	};
	workspace: DetailedGraphWorkspace;
};

type ToolResult = McpUiToolResultNotification["params"];

type Selection =
	| {
			kind: "commit";
			commit: Commit;
			stack: string;
			branch?: string;
	  }
	| {
			kind: "branch";
			reference: DetailedGraphReference;
			stack: string;
	  };

type DetailView =
	| {
			kind: "commit";
			details: CommitDetails;
	  }
	| {
			kind: "branch";
			target?: string | null;
			details: {
				name: string;
				reference: string;
				remoteTrackingBranch?: string | null;
				tip: string;
				pushStatus?: PushStatus | null;
				lastUpdatedAt?: number | null;
				commits: number;
				isConflicted: boolean;
			};
	  };

type GraphStatus = "integrated" | "local" | "remote";

function titleFromCommitMessage(message: string): string {
	return message.split("\n", 1)[0]?.trim() || "(no message)";
}

function commitBody(message: string): string | null {
	const body = message.split("\n").slice(1).join("\n").trim();
	return body || null;
}

function pushStatusLabel(status: PushStatus | undefined): string {
	switch (status) {
		case "nothingToPush":
			return "Nothing to push";
		case "unpushedCommits":
			return "Some unpushed";
		case "unpushedCommitsRequiringForce":
			return "Force push needed";
		case "completelyUnpushed":
			return "Unpushed branch";
		case "integrated":
			return "Integrated";
		case undefined:
			return "Branch";
	}
}

function graphStatusFromPushStatus(status: PushStatus | undefined): GraphStatus {
	switch (status) {
		case "nothingToPush":
			return "remote";
		case "unpushedCommits":
		case "unpushedCommitsRequiringForce":
		case "completelyUnpushed":
			return "local";
		case "integrated":
			return "integrated";
		case undefined:
			return "local";
	}
}

function selectionKey(selection: Selection): string {
	return selection.kind === "commit"
		? `commit:${selection.commit.id}`
		: `branch:${selection.reference.refName.fullName}`;
}

function selectionIdentifier(selection: Selection): string {
	return selection.kind === "commit"
		? selection.commit.id
		: selection.reference.refName.displayName;
}

function selectionContext(view: WorkspaceView, selection: Selection) {
	const identity =
		selection.kind === "commit"
			? {
					kind: selection.kind,
					commitId: selection.commit.id,
					changeId: selection.commit.changeId,
					title: titleFromCommitMessage(selection.commit.message),
					branch: selection.branch,
					stack: selection.stack,
				}
			: {
					kind: selection.kind,
					branch: selection.reference.refName.displayName,
					fullReference: selection.reference.refName.fullName,
					stack: selection.stack,
				};
	const label =
		selection.kind === "commit"
			? `commit ${selection.commit.id} (“${titleFromCommitMessage(selection.commit.message)}”)`
			: `branch ${selection.reference.refName.displayName} (${selection.reference.refName.fullName})`;

	return {
		content: [
			{
				type: "text" as const,
				text: `The user selected ${label} in the GitButler workspace for ${view.repository.path}.`,
			},
		],
		structuredContent: {
			repository: view.repository,
			selection: identity,
		},
	};
}

function textFromToolResult(result: ToolResult): string {
	return (
		result.content?.find((content) => content.type === "text")?.text ??
		"Could not read this workspace."
	);
}

function workspaceViewFromToolResult(result: ToolResult): WorkspaceView | null {
	const value = result.structuredContent;
	if (
		typeof value !== "object" ||
		value === null ||
		!("workspace" in value) ||
		!("repository" in value) ||
		!("summary" in value)
	) {
		return null;
	}
	return value as WorkspaceView;
}

function detailViewFromToolResult(result: ToolResult): DetailView | null {
	const value = result.structuredContent;
	if (typeof value !== "object" || value === null || !("kind" in value) || !("details" in value)) {
		return null;
	}
	if (value.kind === "commit") {
		return value as DetailView;
	}
	if (value.kind === "branch") {
		return value as DetailView;
	}
	return null;
}

function formattedDate(timestamp: number | null | undefined): string | null {
	if (timestamp === null || timestamp === undefined) return null;
	const date = new Date(timestamp);
	if (Number.isNaN(date.getTime())) return null;
	return new Intl.DateTimeFormat(undefined, {
		dateStyle: "medium",
		timeStyle: "short",
	}).format(date);
}

async function copyText(value: string): Promise<void> {
	if (navigator.clipboard?.writeText) {
		await navigator.clipboard.writeText(value);
		return;
	}

	const input = document.createElement("textarea");
	input.value = value;
	input.style.position = "fixed";
	input.style.opacity = "0";
	document.body.appendChild(input);
	input.select();
	const copied = document.execCommand("copy");
	input.remove();
	if (!copied) throw new Error("Clipboard access is unavailable.");
}

function GraphSegment({ kind, status }: { kind: "branch" | "commit"; status: GraphStatus }) {
	return (
		<span className="graph-segment" data-status={status} aria-hidden="true">
			<svg viewBox="0 0 16 28" fill="none">
				{kind === "commit" ? (
					<>
						<path className="graph-rail" d="M8 0V11M8 17V28" />
						<circle cx="8" cy="14" r="3.5" />
					</>
				) : (
					<>
						<path className="graph-rail" d="M8 0V28" />
						<path d="M16 14H14C10.6863 14 8 16.6863 8 20" />
					</>
				)}
			</svg>
		</span>
	);
}

function CommitRow({
	commit,
	selected,
	onSelect,
}: {
	commit: Commit;
	selected: boolean;
	onSelect: () => void;
}) {
	const graphStatus: GraphStatus =
		commit.state.type === "Integrated"
			? "integrated"
			: commit.state.type === "LocalAndRemote"
				? "remote"
				: "local";

	return (
		<button
			className="outline-row commit-row"
			data-selected={selected || undefined}
			onClick={onSelect}
			type="button"
		>
			<GraphSegment kind="commit" status={graphStatus} />
			<span className="row-label" title={commit.message}>
				{titleFromCommitMessage(commit.message)}
			</span>
			<span className="row-meta">
				{commit.hasConflicts && <span className="conflict-badge">Conflict</span>}
				<code>{commit.id.slice(0, 7)}</code>
			</span>
		</button>
	);
}

function BranchRow({
	reference,
	selected,
	onSelect,
}: {
	reference: DetailedGraphReference;
	selected: boolean;
	onSelect: () => void;
}) {
	const pushStatus = reference.status?.pushStatus;

	return (
		<button
			className="outline-row branch-row"
			data-selected={selected || undefined}
			onClick={onSelect}
			type="button"
		>
			<GraphSegment kind="branch" status={graphStatusFromPushStatus(pushStatus)} />
			<span className="branch-label">
				<strong title={reference.refName.fullName}>{reference.refName.displayName}</strong>
				<small>{pushStatusLabel(pushStatus)}</small>
			</span>
			<span className="branch-kind">Branch</span>
		</button>
	);
}

function branchForRow(stack: DetailedGraphStack, rowIndex: number): string | undefined {
	const segment = stack.referenceSegments.find((candidate) => candidate.rowIdxs.includes(rowIndex));
	if (segment === undefined) return undefined;
	const referenceRow = stack.rows[segment.referenceIdx];
	return referenceRow?.data.type === "Reference"
		? referenceRow.data.subject.refName.displayName
		: undefined;
}

function WorkspaceRow({
	row,
	rowIndex,
	stack,
	stackLabel,
	selection,
	onSelect,
}: {
	row: DetailedGraphRow;
	rowIndex: number;
	stack: DetailedGraphStack;
	stackLabel: string;
	selection: Selection | null;
	onSelect: (selection: Selection) => void;
}) {
	if (row.data.type === "Reference") {
		const nextSelection: Selection = {
			kind: "branch",
			reference: row.data.subject,
			stack: stackLabel,
		};
		return (
			<BranchRow
				reference={row.data.subject}
				selected={selection !== null && selectionKey(selection) === selectionKey(nextSelection)}
				onSelect={() => onSelect(nextSelection)}
			/>
		);
	}

	const nextSelection: Selection = {
		kind: "commit",
		commit: row.data.subject,
		stack: stackLabel,
		branch: branchForRow(stack, rowIndex),
	};
	return (
		<CommitRow
			commit={row.data.subject}
			selected={selection !== null && selectionKey(selection) === selectionKey(nextSelection)}
			onSelect={() => onSelect(nextSelection)}
		/>
	);
}

function stackName(stack: DetailedGraphStack, index: number): string {
	const reference = stack.rows.find((row) => row.data.type === "Reference");
	return reference?.data.type === "Reference"
		? reference.data.subject.refName.displayName
		: `Stack ${index + 1}`;
}

function StackCard({
	stack,
	index,
	selection,
	onSelect,
}: {
	stack: DetailedGraphStack;
	index: number;
	selection: Selection | null;
	onSelect: (selection: Selection) => void;
}) {
	const [expanded, setExpanded] = useState(true);
	const branchCount = stack.rows.filter((row) => row.data.type === "Reference").length;
	const commitCount = stack.rows.filter((row) => row.data.type === "Commit").length;
	const label = stackName(stack, index);

	return (
		<section className="stack-card">
			<button
				type="button"
				className="stack-header"
				aria-expanded={expanded}
				onClick={() => setExpanded((current) => !current)}
			>
				<svg
					className="chevron"
					data-expanded={expanded || undefined}
					viewBox="0 0 16 16"
					aria-hidden="true"
				>
					<path d="m6 4 4 4-4 4" />
				</svg>
				<span>{label}</span>
				<span className="stack-counts">
					{branchCount} {branchCount === 1 ? "branch" : "branches"} · {commitCount}{" "}
					{commitCount === 1 ? "commit" : "commits"}
				</span>
			</button>

			{expanded && (
				<div className="stack-rows">
					{stack.rows.length > 0 ? (
						stack.rows.map((row, rowIndex) => (
							<WorkspaceRow
								key={
									row.data.type === "Commit"
										? row.data.subject.id
										: `${row.data.subject.refName.fullName}-${rowIndex}`
								}
								row={row}
								rowIndex={rowIndex}
								stack={stack}
								stackLabel={label}
								selection={selection}
								onSelect={onSelect}
							/>
						))
					) : (
						<div className="empty-stack">No branches or commits.</div>
					)}
				</div>
			)}
		</section>
	);
}

function Metric({ value, label }: { value: number; label: string }) {
	return (
		<span className="metric">
			<strong>{value}</strong>
			<small>{label}</small>
		</span>
	);
}

function CopyButton({
	value,
	label,
	copied,
	onCopy,
}: {
	value: string;
	label: string;
	copied: boolean;
	onCopy: (value: string) => void;
}) {
	return (
		<button className="copy-button" onClick={() => onCopy(value)} type="button" title={label}>
			<svg viewBox="0 0 16 16" aria-hidden="true">
				{copied ? (
					<path d="m3 8 3 3 7-7" />
				) : (
					<>
						<rect x="5.25" y="5.25" width="8" height="8" rx="1.5" />
						<path d="M10.75 5.25v-1.5a1 1 0 0 0-1-1h-6a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h1.5" />
					</>
				)}
			</svg>
			<span>{copied ? "Copied" : label}</span>
		</button>
	);
}

function DetailActions({
	canMessage,
	pending,
	onAction,
}: {
	canMessage: boolean;
	pending: "explain" | "review" | null;
	onAction: (action: "explain" | "review") => void;
}) {
	return (
		<footer className="detail-actions">
			<button
				className="detail-button"
				disabled={!canMessage || pending !== null}
				onClick={() => onAction("explain")}
				type="button"
				title={canMessage ? "Ask the agent to explain this selection" : "Host cannot send messages"}
			>
				{pending === "explain" ? "Explaining…" : "Explain"}
			</button>
			<button
				className="detail-button primary"
				disabled={!canMessage || pending !== null}
				onClick={() => onAction("review")}
				type="button"
				title={canMessage ? "Ask the agent to review this selection" : "Host cannot send messages"}
			>
				{pending === "review" ? "Starting review…" : "Review"}
			</button>
		</footer>
	);
}

function CommitDetail({
	selection,
	detail,
	copied,
	onCopy,
}: {
	selection: Extract<Selection, { kind: "commit" }>;
	detail: Extract<DetailView, { kind: "commit" }> | null;
	copied: string | null;
	onCopy: (value: string) => void;
}) {
	const commit = detail?.details.commit ?? selection.commit;
	const stats = detail?.details.stats;
	const body = commitBody(commit.message);
	const authoredAt = formattedDate(commit.authoredAt);

	return (
		<>
			<header className="detail-header">
				<span className="detail-kind">Commit details</span>
				<h2>{titleFromCommitMessage(commit.message)}</h2>
				<div className="identifier-row">
					<code title={commit.id}>{commit.id}</code>
					<CopyButton
						value={commit.id}
						label="Copy SHA"
						copied={copied === commit.id}
						onCopy={onCopy}
					/>
				</div>
			</header>

			{body && <p className="commit-body">{body}</p>}

			<div className="detail-facts">
				<span>{commit.author.name}</span>
				{authoredAt && <span>{authoredAt}</span>}
				<span>{selection.branch ?? selection.stack}</span>
				{commit.hasConflicts && <span className="danger-text">Has conflicts</span>}
			</div>

			{stats && (
				<div className="change-summary" aria-label="Commit change summary">
					<strong>{stats.filesChanged}</strong> files
					<span className="additions">+{stats.linesAdded}</span>
					<span className="deletions">−{stats.linesRemoved}</span>
				</div>
			)}

			{detail && detail.details.changes.length > 0 && (
				<div className="changed-files">
					<span className="section-label">Changed files</span>
					<ul>
						{detail.details.changes.slice(0, 6).map((change) => (
							<li key={change.path}>
								<span>{change.path}</span>
								<small>{change.status.type}</small>
							</li>
						))}
					</ul>
					{detail.details.changes.length > 6 && (
						<small className="more-files">+{detail.details.changes.length - 6} more files</small>
					)}
				</div>
			)}
		</>
	);
}

function BranchDetail({
	selection,
	detail,
	copied,
	onCopy,
}: {
	selection: Extract<Selection, { kind: "branch" }>;
	detail: Extract<DetailView, { kind: "branch" }> | null;
	copied: string | null;
	onCopy: (value: string) => void;
}) {
	const name = detail?.details.name ?? selection.reference.refName.displayName;
	const pushStatus = detail?.details.pushStatus ?? selection.reference.status?.pushStatus;
	const updatedAt = formattedDate(detail?.details.lastUpdatedAt);

	return (
		<>
			<header className="detail-header">
				<span className="detail-kind">Branch details</span>
				<h2>{name}</h2>
				<div className="identifier-row">
					<code title={selection.reference.refName.fullName}>
						{selection.reference.refName.fullName}
					</code>
					<CopyButton value={name} label="Copy branch" copied={copied === name} onCopy={onCopy} />
				</div>
			</header>

			<div className="branch-status">{pushStatusLabel(pushStatus)}</div>

			{detail && (
				<dl className="branch-facts">
					<div>
						<dt>Target</dt>
						<dd>{detail.target ?? "Not configured"}</dd>
					</div>
					<div>
						<dt>Upstream</dt>
						<dd>{detail.details.remoteTrackingBranch ?? "Not published"}</dd>
					</div>
					<div>
						<dt>Commits</dt>
						<dd>{detail.details.commits}</dd>
					</div>
					<div>
						<dt>State</dt>
						<dd>{detail.details.isConflicted ? "Conflicted" : "Clean"}</dd>
					</div>
					<div>
						<dt>Tip</dt>
						<dd>
							<code>{detail.details.tip.slice(0, 7)}</code>
						</dd>
					</div>
					{updatedAt && (
						<div>
							<dt>Updated</dt>
							<dd>{updatedAt}</dd>
						</div>
					)}
				</dl>
			)}
		</>
	);
}

function DetailsPanel({
	selection,
	detail,
	loading,
	error,
	copied,
	canMessage,
	pendingAction,
	onBack,
	onCopy,
	onAction,
}: {
	selection: Selection;
	detail: DetailView | null;
	loading: boolean;
	error: string | null;
	copied: string | null;
	canMessage: boolean;
	pendingAction: "explain" | "review" | null;
	onBack: () => void;
	onCopy: (value: string) => void;
	onAction: (action: "explain" | "review") => void;
}) {
	return (
		<aside className="details-panel">
			<button className="back-button" onClick={onBack} type="button">
				<svg viewBox="0 0 16 16" aria-hidden="true">
					<path d="M10 3 5 8l5 5" />
				</svg>
				Workspace
			</button>

			<div className="details-content">
				{selection.kind === "commit" ? (
					<CommitDetail
						selection={selection}
						detail={detail?.kind === "commit" ? detail : null}
						copied={copied}
						onCopy={onCopy}
					/>
				) : (
					<BranchDetail
						selection={selection}
						detail={detail?.kind === "branch" ? detail : null}
						copied={copied}
						onCopy={onCopy}
					/>
				)}

				{loading && (
					<div className="detail-loading">
						<span className="spinner" aria-hidden="true" />
						Loading repository details…
					</div>
				)}
				{error && (
					<div className="detail-error" role="alert">
						{error}
					</div>
				)}
			</div>

			<DetailActions canMessage={canMessage} pending={pendingAction} onAction={onAction} />
		</aside>
	);
}

function Workspace({ view, app }: { view: WorkspaceView; app: App }) {
	const [selection, setSelection] = useState<Selection | null>(null);
	const [detail, setDetail] = useState<DetailView | null>(null);
	const [detailLoading, setDetailLoading] = useState(false);
	const [detailError, setDetailError] = useState<string | null>(null);
	const [copied, setCopied] = useState<string | null>(null);
	const [pendingAction, setPendingAction] = useState<"explain" | "review" | null>(null);
	const detailRequest = useRef(0);
	const capabilities = app.getHostCapabilities();
	const canCallTools = capabilities?.serverTools !== undefined;
	const canUpdateContext = capabilities?.updateModelContext !== undefined;
	const canMessage = capabilities?.message !== undefined;

	async function select(nextSelection: Selection) {
		const request = ++detailRequest.current;
		setSelection(nextSelection);
		setDetail(null);
		setDetailError(null);
		setPendingAction(null);

		if (canUpdateContext) {
			void app.updateModelContext(selectionContext(view, nextSelection)).catch(() => {
				// The visible selection remains useful when a host rejects context updates.
			});
		}

		if (!canCallTools) {
			setDetailLoading(false);
			setDetailError("This host cannot load additional repository details.");
			return;
		}

		setDetailLoading(true);
		try {
			const result = await app.callServerTool({
				name:
					nextSelection.kind === "commit" ? "gitbutler_commit_details" : "gitbutler_branch_details",
				arguments:
					nextSelection.kind === "commit"
						? {
								repository: view.repository.path,
								commitId: nextSelection.commit.id,
							}
						: {
								repository: view.repository.path,
								branch: nextSelection.reference.refName.fullName,
							},
			});
			if (request !== detailRequest.current) return;
			if (result.isError) throw new Error(textFromToolResult(result));
			const nextDetail = detailViewFromToolResult(result);
			if (nextDetail === null) throw new Error("The detail result was missing structured data.");
			setDetail(nextDetail);
		} catch (caught) {
			if (request !== detailRequest.current) return;
			setDetailError(caught instanceof Error ? caught.message : "Could not load details.");
		} finally {
			if (request === detailRequest.current) setDetailLoading(false);
		}
	}

	async function handleCopy(value: string) {
		setDetailError(null);
		try {
			await copyText(value);
			setCopied(value);
			window.setTimeout(() => setCopied((current) => (current === value ? null : current)), 1600);
		} catch (caught) {
			setDetailError(caught instanceof Error ? caught.message : "Could not copy the identifier.");
		}
	}

	async function handleAction(action: "explain" | "review") {
		if (selection === null || !canMessage) return;
		setPendingAction(action);
		setDetailError(null);
		try {
			if (canUpdateContext) {
				await app.updateModelContext(selectionContext(view, selection));
			}
			const identifier = selectionIdentifier(selection);
			const subject =
				selection.kind === "commit"
					? `commit ${identifier}`
					: `branch ${identifier} against its configured target`;
			const request =
				action === "explain"
					? `Explain ${subject} in repository ${view.repository.path}. Describe its intent and the important changes.`
					: `Review ${subject} in repository ${view.repository.path} for correctness, regressions, and missing tests.`;
			const result = await app.sendMessage({
				role: "user",
				content: [{ type: "text", text: request }],
			});
			if (result.isError) throw new Error("The host rejected the request.");
		} catch (caught) {
			setDetailError(
				caught instanceof Error ? caught.message : "Could not start the agent request.",
			);
		} finally {
			setPendingAction(null);
		}
	}

	function clearSelection() {
		detailRequest.current += 1;
		setSelection(null);
		setDetail(null);
		setDetailError(null);
		setDetailLoading(false);
	}

	return (
		<main className="workspace-shell">
			<header className="workspace-header">
				<div className="repository">
					<span className="eyebrow">GitButler workspace</span>
					<h1>{view.repository.name}</h1>
					<p title={view.repository.path}>{view.repository.path}</p>
				</div>

				<div className="summary" aria-label="Workspace summary">
					<Metric value={view.summary.stacks} label="Stacks" />
					<Metric value={view.summary.branches} label="Branches" />
					<Metric value={view.summary.commits} label="Commits" />
				</div>
			</header>

			<div className="workspace-explorer" data-has-selection={selection !== null || undefined}>
				<div className="workspace-outline">
					<div className="stacks">
						{view.workspace.stacks.length > 0 ? (
							view.workspace.stacks.map((stack, index) => (
								<StackCard
									key={`${stackName(stack, index)}-${index}`}
									stack={stack}
									index={index}
									selection={selection}
									onSelect={(nextSelection) => void select(nextSelection)}
								/>
							))
						) : (
							<div className="empty-state">This workspace has no stacks yet.</div>
						)}
					</div>
				</div>

				{selection && (
					<DetailsPanel
						selection={selection}
						detail={detail}
						loading={detailLoading}
						error={detailError}
						copied={copied}
						canMessage={canMessage}
						pendingAction={pendingAction}
						onBack={clearSelection}
						onCopy={(value) => void handleCopy(value)}
						onAction={(action) => void handleAction(action)}
					/>
				)}
			</div>
		</main>
	);
}

export function WorkspaceApp() {
	const [toolResult, setToolResult] = useState<ToolResult | null>(null);
	const { app, isConnected, error } = useApp({
		appInfo: { name: "GitButler workspace", version: "1.0.0" },
		capabilities: {},
		onAppCreated: (createdApp) => {
			createdApp.addEventListener("toolresult", setToolResult);
		},
	});
	useHostStyles(app, app?.getHostContext());

	if (error !== null) {
		return (
			<div className="message-state error-state">
				Could not connect to the host: {error.message}
			</div>
		);
	}
	if (!isConnected || toolResult === null || app === null) {
		return (
			<div className="message-state loading-state">
				<span className="spinner" aria-hidden="true" />
				Loading GitButler workspace…
			</div>
		);
	}
	if (toolResult.isError) {
		return <div className="message-state error-state">{textFromToolResult(toolResult)}</div>;
	}

	const view = workspaceViewFromToolResult(toolResult);
	if (view === null) {
		return (
			<div className="message-state error-state">
				The workspace result did not contain structured data.
			</div>
		);
	}

	return <Workspace view={view} app={app} />;
}

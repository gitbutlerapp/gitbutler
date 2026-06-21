import { Button } from "#ui/components/ui/button.tsx";
import { getHowStatus, howStatusQueryKey } from "#ui/lib/how-status-query.ts";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import {
	AlertCircle,
	Bookmark as BookmarkIcon,
	Check,
	Clock,
	Eye,
	FolderOpen,
	GitCommitVertical,
	MoreHorizontal,
	Pencil,
	Plus,
	RefreshCw,
	Settings,
	Trash2,
	Upload,
} from "lucide-react";
import { memo, useCallback, useEffect, useRef, useState } from "react";
import type {
	Bookmark,
	BrowsingSession,
	Checkpoint,
	GithubRepositorySummary,
	HowStatus,
	SaveState,
} from "../../../electron/src/ipc";

const initialStatus: HowStatus = {
	project: null,
	saveState: "idle",
	message: null,
	lastSavedAt: null,
	checkpoints: [],
	bookmarks: [],
	browsing: null,
	settings: {
		checkpointDebounceMs: 10_000,
		codingAgent: "none",
	},
};

function statusLabel(status: HowStatus): string {
	if (status.message) return status.message;
	switch (status.saveState) {
		case "idle":
			return "Open or start a project";
		case "watching":
			return "Watching for changes";
		case "pending":
			return "Saving soon";
		case "saving":
			return "Saving";
		case "saved":
			return "Saved";
		case "error":
			return "Could not save";
	}
}

function statusTone(state: SaveState): string {
	switch (state) {
		case "saved":
			return "bg-emerald-100 text-emerald-900";
		case "error":
			return "bg-red-100 text-red-900";
		case "saving":
		case "pending":
			return "bg-stone-100 text-stone-500";
		case "idle":
		case "watching":
			return "bg-stone-100 text-stone-400";
	}
}

function formatTime(timestamp: number): string {
	return new Intl.DateTimeFormat(undefined, {
		month: "short",
		day: "numeric",
		hour: "2-digit",
		minute: "2-digit",
	}).format(new Date(timestamp));
}

function checkpointDisplayTitle(title: string): string {
	return title.replace(/^Checkpoint:\s*/i, "");
}

function checkpointTimelineKey(checkpoint: Checkpoint): string {
	return checkpoint.changeId ?? checkpoint.id;
}

function iconForState(state: SaveState) {
	if (state === "saved") return <Check className="h-4 w-4" aria-hidden />;
	if (state === "saving" || state === "pending")
		return <RefreshCw className="h-4 w-4 animate-spin" aria-hidden />;
	if (state === "error") return <AlertCircle className="h-4 w-4" aria-hidden />;
	return <Clock className="h-4 w-4" aria-hidden />;
}

function EmptyState({
	onOpen,
	onStart,
	busy,
	error,
}: {
	onOpen: () => Promise<void>;
	onStart: () => Promise<void>;
	busy: boolean;
	error: string | null;
}) {
	return (
		<main className="flex min-h-screen items-center justify-center px-6 py-10">
			<section className="w-full max-w-xl">
				<div className="mb-10">
					<h1 className="text-4xl font-semibold tracking-normal text-stone-950">Manage changes.</h1>
					<p className="mt-4 max-w-md text-base leading-7 text-stone-600">
						Open a project and How will keep a simple timeline of saved moments while you build.
					</p>
				</div>

				<div className="flex flex-wrap gap-3">
					<Button onClick={() => void onOpen()} disabled={busy}>
						<FolderOpen className="h-4 w-4" aria-hidden />
						Open project
					</Button>
					<Button variant="secondary" onClick={() => void onStart()} disabled={busy}>
						<Plus className="h-4 w-4" aria-hidden />
						Start project
					</Button>
				</div>

				{error ? (
					<p className="mt-5 rounded-md border border-red-200 bg-red-50 px-3 py-2 text-sm text-red-900">
						{error}
					</p>
				) : null}
			</section>
		</main>
	);
}

type PendingDirtyAction =
	| { type: "view"; checkpoint: Checkpoint }
	| { type: "switchBookmark"; bookmark: Bookmark }
	| { type: "returnToLatest" }
	| { type: "open" }
	| { type: "start" }
	| { type: "delete" };

type BookmarkNameAction = { type: "create" } | { type: "rename"; bookmark: Bookmark };
type BookmarkConfirmAction =
	| { type: "update"; bookmark: Bookmark }
	| { type: "delete"; bookmark: Bookmark };

type PendingAction =
	| "openProject"
	| "startProject"
	| "deleteProject"
	| "viewCheckpoint"
	| "createBookmark"
	| "updateBookmark"
	| "renameBookmark"
	| "deleteBookmark"
	| "switchBookmark"
	| "continueFromCheckpoint"
	| "returnToLatest"
	| "publish"
	| "githubLogin"
	| "githubCreateRepository"
	| "githubLoadRepositories"
	| "githubPublishRepository"
	| "leaveBrowsingChanges";

function DirtyBrowsingDialog({
	onLeave,
	onCancel,
}: {
	onLeave: () => Promise<void>;
	onCancel: () => void;
}) {
	return (
		<div className="fixed inset-0 z-50 flex items-center justify-center bg-stone-950/20 px-4">
			<section className="w-full max-w-sm rounded-md border border-stone-200 bg-white p-5 shadow-lg">
				<h2 className="text-base font-semibold tracking-normal text-stone-950">Leave changes?</h2>
				<p className="mt-2 text-sm leading-6 text-stone-600">
					You changed files while browsing this checkpoint. To keep those changes, choose Continue
					from here first.
				</p>
				<div className="mt-5 flex justify-end gap-2">
					<Button variant="ghost" onClick={onCancel}>
						Cancel
					</Button>
					<Button onClick={() => void onLeave()}>Leave changes</Button>
				</div>
			</section>
		</div>
	);
}

function BookmarkNameDialog({
	title,
	initialName,
	onSave,
	onCancel,
	busy,
}: {
	title: string;
	initialName: string;
	onSave: (name: string) => Promise<void>;
	onCancel: () => void;
	busy: boolean;
}) {
	const [name, setName] = useState(initialName);
	return (
		<div className="fixed inset-0 z-50 flex items-center justify-center bg-stone-950/20 px-4">
			<section className="w-full max-w-sm rounded-md border border-stone-200 bg-white p-5 shadow-lg">
				<h2 className="text-base font-semibold tracking-normal text-stone-950">{title}</h2>
				<label className="mt-4 block text-sm font-medium text-stone-950">
					Name
					<input
						className="mt-2 h-10 w-full rounded-md border border-stone-300 bg-white px-3 text-sm text-stone-950 outline-none focus:border-stone-500"
						value={name}
						onChange={(event) => setName(event.target.value)}
						disabled={busy}
						autoFocus
					/>
				</label>
				<div className="mt-5 flex justify-end gap-2">
					<Button variant="ghost" onClick={onCancel} disabled={busy}>
						Cancel
					</Button>
					<Button
						onClick={() => void onSave(name.trim())}
						disabled={busy || name.trim().length === 0}
					>
						Save
					</Button>
				</div>
			</section>
		</div>
	);
}

function BookmarkConfirmDialog({
	title,
	body,
	action,
	onConfirm,
	onCancel,
	busy,
}: {
	title: string;
	body: string;
	action: string;
	onConfirm: () => Promise<void>;
	onCancel: () => void;
	busy: boolean;
}) {
	return (
		<div className="fixed inset-0 z-50 flex items-center justify-center bg-stone-950/20 px-4">
			<section className="w-full max-w-sm rounded-md border border-stone-200 bg-white p-5 shadow-lg">
				<h2 className="text-base font-semibold tracking-normal text-stone-950">{title}</h2>
				<p className="mt-2 text-sm leading-6 text-stone-600">{body}</p>
				<div className="mt-5 flex justify-end gap-2">
					<Button variant="ghost" onClick={onCancel} disabled={busy}>
						Cancel
					</Button>
					<Button onClick={() => void onConfirm()} disabled={busy}>
						{action}
					</Button>
				</div>
			</section>
		</div>
	);
}

function GithubLoginDialog({
	onLogin,
	onCancel,
	busy,
}: {
	onLogin: () => Promise<void>;
	onCancel: () => void;
	busy: boolean;
}) {
	return (
		<div className="fixed inset-0 z-50 flex items-center justify-center bg-stone-950/20 px-4">
			<section className="w-full max-w-lg rounded-md border border-stone-200 bg-white p-5 shadow-lg">
				<h2 className="text-base font-semibold tracking-normal text-stone-950">
					Publish with GitHub
				</h2>
				<p className="mt-2 text-sm leading-6 text-stone-600">
					Log in to choose where this project publishes.
				</p>
				<div className="mt-5 flex flex-wrap justify-end gap-2">
					<Button variant="ghost" onClick={onCancel} disabled={busy}>
						Cancel
					</Button>
					<Button onClick={() => void onLogin()} disabled={busy}>
						Log in to GitHub
					</Button>
				</div>
			</section>
		</div>
	);
}

function GithubRepositoryChoiceDialog({
	onCreate,
	onChoose,
	onCancel,
	busy,
}: {
	onCreate: () => void;
	onChoose: () => Promise<void>;
	onCancel: () => void;
	busy: boolean;
}) {
	return (
		<div className="fixed inset-0 z-50 flex items-center justify-center bg-stone-950/20 px-4">
			<section className="w-full max-w-xl rounded-md border border-stone-200 bg-white p-5 shadow-lg">
				<h2 className="text-base font-semibold tracking-normal text-stone-950">
					Where should this publish?
				</h2>
				<p className="mt-2 text-sm leading-6 text-stone-600">
					Create a new GitHub project or choose one you already have.
				</p>
				<div className="mt-5 flex flex-wrap justify-end gap-2">
					<Button variant="ghost" onClick={onCancel} disabled={busy}>
						Cancel
					</Button>
					<Button variant="ghost" onClick={() => void onChoose()} disabled={busy}>
						Choose existing project
					</Button>
					<Button onClick={onCreate} disabled={busy}>
						Create GitHub project
					</Button>
				</div>
			</section>
		</div>
	);
}

function CreateGithubRepositoryDialog({
	defaultName,
	onPublish,
	onCancel,
	busy,
}: {
	defaultName: string;
	onPublish: (name: string) => Promise<void>;
	onCancel: () => void;
	busy: boolean;
}) {
	const [name, setName] = useState(defaultName);
	return (
		<div className="fixed inset-0 z-50 flex items-center justify-center bg-stone-950/20 px-4">
			<section className="w-full max-w-md rounded-md border border-stone-200 bg-white p-5 shadow-lg">
				<h2 className="text-base font-semibold tracking-normal text-stone-950">
					Create GitHub project
				</h2>
				<label className="mt-4 block text-sm font-medium text-stone-950">
					Project name
					<input
						className="mt-2 h-10 w-full rounded-md border border-stone-300 bg-white px-3 text-sm text-stone-950 outline-none focus:border-stone-500"
						value={name}
						onChange={(event) => setName(event.target.value)}
						disabled={busy}
					/>
				</label>
				<div className="mt-5 flex justify-end gap-2">
					<Button variant="ghost" onClick={onCancel} disabled={busy}>
						Cancel
					</Button>
					<Button onClick={() => void onPublish(name)} disabled={busy || name.trim().length === 0}>
						Create and publish
					</Button>
				</div>
			</section>
		</div>
	);
}

function ChooseGithubRepositoryDialog({
	repositories,
	onPublish,
	onCancel,
	busy,
}: {
	repositories: Array<GithubRepositorySummary>;
	onPublish: (repository: GithubRepositorySummary) => Promise<void>;
	onCancel: () => void;
	busy: boolean;
}) {
	const [query, setQuery] = useState("");
	const filtered = repositories.filter((repository) =>
		repository.nameWithOwner.toLowerCase().includes(query.toLowerCase()),
	);
	return (
		<div className="fixed inset-0 z-50 flex items-center justify-center bg-stone-950/20 px-4">
			<section className="w-full max-w-md rounded-md border border-stone-200 bg-white p-5 shadow-lg">
				<h2 className="text-base font-semibold tracking-normal text-stone-950">
					Choose existing project
				</h2>
				<input
					aria-label="Search GitHub projects"
					className="mt-4 h-10 w-full rounded-md border border-stone-300 bg-white px-3 text-sm text-stone-950 outline-none focus:border-stone-500"
					value={query}
					onChange={(event) => setQuery(event.target.value)}
					placeholder="Search"
					disabled={busy}
				/>
				<div className="mt-3 max-h-72 space-y-2 overflow-auto">
					{filtered.map((repository) => (
						<button
							key={repository.id}
							className="block w-full rounded-md border border-stone-200 px-3 py-2 text-left text-sm font-medium text-stone-900 hover:bg-stone-50"
							onClick={() => void onPublish(repository)}
							disabled={busy}
						>
							{repository.nameWithOwner}
						</button>
					))}
					{filtered.length === 0 ? (
						<p className="py-5 text-center text-sm text-stone-500">No projects found</p>
					) : null}
				</div>
				<div className="mt-5 flex justify-end">
					<Button variant="ghost" onClick={onCancel} disabled={busy}>
						Cancel
					</Button>
				</div>
			</section>
		</div>
	);
}

function BookmarkSidebar({
	bookmarks,
	browsing,
	highlightedBookmarkIds,
	onCreate,
	onSwitch,
	onUpdate,
	onRename,
	onDelete,
	busy,
}: {
	bookmarks: Array<Bookmark>;
	browsing: BrowsingSession | null;
	highlightedBookmarkIds: Set<string>;
	onCreate: () => Promise<void>;
	onSwitch: (bookmark: Bookmark) => Promise<void>;
	onUpdate: (bookmark: Bookmark) => Promise<void>;
	onRename: (bookmark: Bookmark) => Promise<void>;
	onDelete: (bookmark: Bookmark) => Promise<void>;
	busy: boolean;
}) {
	const [openMenuBookmarkId, setOpenMenuBookmarkId] = useState<string | null>(null);

	function closeMenu() {
		setOpenMenuBookmarkId(null);
	}

	return (
		<aside className="flex w-full shrink-0 flex-col border-stone-200 lg:h-full lg:min-h-0 lg:w-64 lg:border-r lg:pr-5">
			<div className="mb-3 flex shrink-0 items-center justify-between gap-2">
				<h2 className="text-sm font-semibold tracking-normal text-stone-950">Bookmarks</h2>
				<Button
					variant="ghost"
					size="icon"
					aria-label="Bookmark current state"
					onClick={() => void onCreate()}
					disabled={busy || Boolean(browsing?.dirty)}
					title={
						browsing?.dirty ? "Continue from here before bookmarking these changes." : undefined
					}
				>
					<BookmarkIcon className="h-4 w-4" aria-hidden />
				</Button>
			</div>

			<div className="pr-1 lg:min-h-0 lg:flex-1 lg:overflow-y-auto">
				{bookmarks.length === 0 ? (
					<div className="rounded-md border border-dashed border-stone-300 bg-white/70 p-4">
						<p className="text-sm font-medium text-stone-900">No bookmarks</p>
						<Button
							variant="secondary"
							size="sm"
							className="mt-3 w-full"
							onClick={() => void onCreate()}
							disabled={busy || Boolean(browsing?.dirty)}
						>
							<BookmarkIcon className="h-4 w-4" aria-hidden />
							Bookmark current state
						</Button>
					</div>
				) : (
					<ul className="space-y-2">
						{bookmarks.map((bookmark) => (
							<li
								key={bookmark.id}
								className={`relative rounded-md border p-2 ${
									bookmark.isCurrent ? "border-stone-500 bg-white" : "border-stone-200 bg-stone-100"
								} ${highlightedBookmarkIds.has(bookmark.id) ? "checkpoint-message-flash" : ""}`}
							>
								<div className="flex items-start gap-2">
									<button
										className="block min-w-0 flex-1 text-left disabled:cursor-default"
										onClick={() => void onSwitch(bookmark)}
										disabled={busy || bookmark.isCurrent}
									>
										<span className="block truncate text-sm font-medium text-stone-950">
											{bookmark.name}
										</span>
										<span className="mt-1 flex items-center gap-2 text-xs text-stone-500">
											{bookmark.isCurrent ? "current" : formatTime(bookmark.updatedAt)}
											{bookmark.kind === "auto" ? <span>auto</span> : null}
										</span>
									</button>
									<Button
										variant="ghost"
										size="icon"
										className="h-7 w-7 shrink-0"
										aria-label={`More actions for ${bookmark.name}`}
										aria-expanded={openMenuBookmarkId === bookmark.id}
										onClick={() =>
											setOpenMenuBookmarkId((current) =>
												current === bookmark.id ? null : bookmark.id,
											)
										}
										disabled={busy}
									>
										<MoreHorizontal className="h-4 w-4" aria-hidden />
									</Button>
								</div>
								{openMenuBookmarkId === bookmark.id ? (
									<div className="absolute right-2 top-10 z-10 w-36 rounded-md border border-stone-200 bg-white p-1 shadow-lg">
										<button
											className="flex h-8 w-full items-center gap-2 rounded px-2 text-left text-sm text-stone-700 hover:bg-stone-100 disabled:cursor-not-allowed disabled:text-stone-400"
											onClick={() => {
												closeMenu();
												void onRename(bookmark);
											}}
											disabled={busy}
										>
											<Pencil className="h-3.5 w-3.5" aria-hidden />
											Rename
										</button>
										<button
											className="flex h-8 w-full items-center gap-2 rounded px-2 text-left text-sm text-stone-700 hover:bg-stone-100 disabled:cursor-not-allowed disabled:text-stone-400"
											onClick={() => {
												closeMenu();
												void onUpdate(bookmark);
											}}
											disabled={busy || Boolean(browsing)}
										>
											<RefreshCw className="h-3.5 w-3.5" aria-hidden />
											Update
										</button>
										<button
											className="flex h-8 w-full items-center gap-2 rounded px-2 text-left text-sm text-red-700 hover:bg-red-50 disabled:cursor-not-allowed disabled:text-stone-400"
											onClick={() => {
												closeMenu();
												void onDelete(bookmark);
											}}
											disabled={busy}
										>
											<Trash2 className="h-3.5 w-3.5" aria-hidden />
											Delete
										</button>
									</div>
								) : null}
							</li>
						))}
					</ul>
				)}
			</div>
		</aside>
	);
}

function Timeline({
	checkpoints,
	browsing,
	highlightedCheckpointKeys,
	onView,
	busy,
}: {
	checkpoints: Array<Checkpoint>;
	browsing: BrowsingSession | null;
	highlightedCheckpointKeys: Set<string>;
	onView: (checkpoint: Checkpoint) => Promise<void>;
	busy: boolean;
}) {
	if (checkpoints.length === 0)
		return (
			<div className="rounded-md border border-dashed border-stone-300 bg-white/70 p-8 text-center">
				<p className="text-sm font-medium text-stone-900">No checkpoints yet</p>
				<p className="mt-2 text-sm leading-6 text-stone-500">
					Make a change in your editor. How will save after things are quiet for a moment.
				</p>
			</div>
		);

	return (
		<ol className="space-y-3">
			{checkpoints.map((checkpoint, index) => (
				<li
					key={checkpointTimelineKey(checkpoint)}
					className={`group grid grid-cols-[auto_1fr_auto] gap-4 rounded-md border px-4 py-3 ${
						browsing?.currentCheckpointId === checkpoint.id
							? "border-stone-500 bg-white"
							: "border-stone-200 bg-stone-100"
					} ${
						highlightedCheckpointKeys.has(checkpointTimelineKey(checkpoint))
							? "checkpoint-message-flash"
							: ""
					}`}
				>
					<div className="flex flex-col justify-center">
						<GitCommitVertical className="mt-0.5 h-5 w-5 text-stone-500" aria-hidden />
					</div>
					<div className="min-w-0 flex-1">
						<div className="flex min-w-0 items-center gap-2">
							<p className="truncate text-sm font-medium text-stone-950">
								{checkpointDisplayTitle(checkpoint.title)}
							</p>
							{browsing?.currentCheckpointId === checkpoint.id ? (
								<span className="shrink-0 rounded-md bg-stone-200 px-2 py-0.5 text-xs font-medium text-stone-700">
									viewing
								</span>
							) : null}
						</div>
						<p className="mt-1 text-xs text-stone-500">{formatTime(checkpoint.createdAt)}</p>
					</div>
					{(index === 0 && !browsing) || browsing?.currentCheckpointId === checkpoint.id ? null : (
						<Button
							variant="ghost"
							size="sm"
							className="invisible self-center group-hover:visible group-focus-within:visible"
							onClick={() => void onView(checkpoint)}
							disabled={busy}
						>
							<Eye className="h-4 w-4" aria-hidden />
							view
						</Button>
					)}
				</li>
			))}
		</ol>
	);
}

const PublishButton = memo(
	function PublishButton({
		disabled,
		label,
		title,
		onPublish,
	}: {
		disabled: boolean;
		label: string;
		title?: string;
		onPublish: () => Promise<void>;
	}) {
		return (
			<Button onClick={() => void onPublish()} disabled={disabled} title={title}>
				<Upload className="h-4 w-4" aria-hidden />
				{label}
			</Button>
		);
	},
	(previous, next) =>
		previous.disabled === next.disabled &&
		previous.label === next.label &&
		previous.title === next.title,
);

const BrowsingActions = memo(
	function BrowsingActions({
		disabled,
		onReturnToLatest,
		onContinue,
	}: {
		disabled: boolean;
		onReturnToLatest: () => Promise<void>;
		onContinue: () => Promise<void>;
	}) {
		return (
			<div className="flex flex-wrap gap-2">
				<Button
					variant="secondary"
					onClick={() => void onReturnToLatest()}
					disabled={disabled}
				>
					Return to latest
				</Button>
				<Button onClick={() => void onContinue()} disabled={disabled}>
					Continue from here
				</Button>
			</div>
		);
	},
	(previous, next) => previous.disabled === next.disabled,
);

function ProjectScreen({
	status,
	highlightedCheckpointKeys,
	highlightedBookmarkIds,
	pendingAction,
	onOpen,
	onStart,
	onDelete,
	onPublish,
	onCreateBookmark,
	onSwitchBookmark,
	onUpdateBookmark,
	onRenameBookmark,
	onDeleteBookmark,
	onView,
	onContinue,
	onReturnToLatest,
}: {
	status: HowStatus;
	highlightedCheckpointKeys: Set<string>;
	highlightedBookmarkIds: Set<string>;
	pendingAction: PendingAction | null;
	onOpen: () => Promise<void>;
	onStart: () => Promise<void>;
	onDelete: () => Promise<void>;
	onPublish: () => Promise<void>;
	onCreateBookmark: () => Promise<void>;
	onSwitchBookmark: (bookmark: Bookmark) => Promise<void>;
	onUpdateBookmark: (bookmark: Bookmark) => Promise<void>;
	onRenameBookmark: (bookmark: Bookmark) => Promise<void>;
	onDeleteBookmark: (bookmark: Bookmark) => Promise<void>;
	onView: (checkpoint: Checkpoint) => Promise<void>;
	onContinue: () => Promise<void>;
	onReturnToLatest: () => Promise<void>;
}) {
	const project = status.project;
	if (!project) return null;
	const chromeBusy =
		pendingAction === "openProject" ||
		pendingAction === "startProject" ||
		pendingAction === "deleteProject";
	const bookmarkBusy =
		pendingAction === "createBookmark" ||
		pendingAction === "updateBookmark" ||
		pendingAction === "renameBookmark" ||
		pendingAction === "deleteBookmark";
	const browsingBusy =
		pendingAction === "viewCheckpoint" ||
		pendingAction === "continueFromCheckpoint" ||
		pendingAction === "returnToLatest" ||
		pendingAction === "leaveBrowsingChanges";
	const publishBusy =
		pendingAction === "publish" ||
		pendingAction === "githubLogin" ||
		pendingAction === "githubCreateRepository" ||
		pendingAction === "githubLoadRepositories" ||
		pendingAction === "githubPublishRepository";

	return (
		<main className="min-h-screen px-6 py-6 lg:flex lg:h-screen lg:min-h-0 lg:flex-col lg:overflow-hidden">
			<div className="mx-auto flex w-full max-w-7xl flex-col justify-start gap-6 lg:h-full lg:min-h-0">
				<nav className="shrink-0">
					<Button variant="ghost" size="sm" onClick={() => void onOpen()} disabled={chromeBusy}>
						<FolderOpen className="h-4 w-4" aria-hidden />
						Open
					</Button>
					<Button variant="ghost" size="sm" onClick={() => void onStart()} disabled={chromeBusy}>
						<Plus className="h-4 w-4" aria-hidden />
						Start
					</Button>
					<Button variant="ghost" size="sm" onClick={() => void onDelete()} disabled={chromeBusy}>
						<Trash2 className="h-4 w-4" aria-hidden />
						Delete
					</Button>
				</nav>
				<header className="flex shrink-0 flex-wrap items-start justify-between gap-4 pb-3">
					<div className="min-w-0 flex-1">
						<div className="flex min-w-0 items-center gap-2">
							<h1 className="truncate text-xl font-semibold tracking-normal text-stone-700">
								{project.title}
							</h1>
							<Button asChild variant="ghost" size="icon" aria-label="Project settings">
								<Link to="/settings">
									<Settings className="h-4 w-4" aria-hidden />
								</Link>
							</Button>
						</div>
					</div>
					<div className="flex items-center gap-2">
						<PublishButton
							onPublish={onPublish}
							disabled={publishBusy || Boolean(status.browsing)}
							title={
								status.browsing
									? "Continue from here or return to latest before publishing."
									: undefined
							}
							label={status.message === "Publishing" ? "Publishing" : "Publish"}
						/>
						<span
							className={`inline-flex h-8 items-center gap-2 rounded-md px-3 text-xs font-medium ${statusTone(
								status.saveState,
							)}`}
						>
							{iconForState(status.saveState)}
							{statusLabel(status)}
						</span>
					</div>
				</header>

				{status.browsing ? (
					<section className="flex shrink-0 flex-wrap items-center justify-between gap-3 rounded-md border border-stone-200 bg-white px-4 py-3">
						<p className="text-sm text-stone-600">You are viewing an earlier checkpoint.</p>
						<BrowsingActions
							disabled={browsingBusy}
							onReturnToLatest={onReturnToLatest}
							onContinue={onContinue}
						/>
					</section>
				) : null}

				<div className="flex flex-col gap-6 md:grid md:min-h-0 md:flex-1 md:grid-cols-[16rem_minmax(0,1fr)]">
					<BookmarkSidebar
						bookmarks={status.bookmarks}
						browsing={status.browsing}
						highlightedBookmarkIds={highlightedBookmarkIds}
						onCreate={onCreateBookmark}
						onSwitch={onSwitchBookmark}
						onUpdate={onUpdateBookmark}
						onRename={onRenameBookmark}
						onDelete={onDeleteBookmark}
						busy={bookmarkBusy}
					/>
					<section className="min-w-0 lg:min-h-0 lg:overflow-y-auto">
						<div className="mx-auto w-full max-w-3xl pb-8">
							<Timeline
								checkpoints={status.checkpoints}
								browsing={status.browsing}
								highlightedCheckpointKeys={highlightedCheckpointKeys}
								onView={onView}
								busy={browsingBusy}
							/>
						</div>
					</section>
				</div>
			</div>
		</main>
	);
}

export function HowHome() {
	const queryClient = useQueryClient();
	const statusQuery = useQuery({
		queryKey: howStatusQueryKey,
		queryFn: getHowStatus,
		placeholderData: initialStatus,
	});
	const status = statusQuery.data ?? initialStatus;
	const setStatus = useCallback(
		(nextStatus: HowStatus | ((currentStatus: HowStatus) => HowStatus)) => {
			queryClient.setQueryData<HowStatus>(howStatusQueryKey, (currentStatus) => {
				if (typeof nextStatus === "function") return nextStatus(currentStatus ?? initialStatus);
				return nextStatus;
			});
		},
		[queryClient],
	);
	const [pendingAction, setPendingAction] = useState<PendingAction | null>(null);
	const [error, setError] = useState<string | null>(null);
	const [highlightedCheckpointKeys, setHighlightedCheckpointKeys] = useState<Set<string>>(
		() => new Set(),
	);
	const [highlightedBookmarkIds, setHighlightedBookmarkIds] = useState<Set<string>>(
		() => new Set(),
	);
	const [pendingDirtyAction, setPendingDirtyAction] = useState<PendingDirtyAction | null>(null);
	const [bookmarkNameAction, setBookmarkNameAction] = useState<BookmarkNameAction | null>(null);
	const [bookmarkConfirmAction, setBookmarkConfirmAction] = useState<BookmarkConfirmAction | null>(
		null,
	);
	const [showGithubLoginDialog, setShowGithubLoginDialog] = useState(false);
	const [showGithubRepositoryChoiceDialog, setShowGithubRepositoryChoiceDialog] = useState(false);
	const [showCreateGithubRepositoryDialog, setShowCreateGithubRepositoryDialog] = useState(false);
	const [showChooseGithubRepositoryDialog, setShowChooseGithubRepositoryDialog] = useState(false);
	const [githubRepositoryName, setGithubRepositoryName] = useState("how-project");
	const [githubRepositories, setGithubRepositories] = useState<Array<GithubRepositorySummary>>([]);
	const previousCheckpointTitles = useRef<Map<string, string> | null>(null);
	const previousBookmarkIds = useRef<Set<string> | null>(null);
	const previousBookmarkProjectId = useRef<string | null>(null);
	const checkpointHighlightTimers = useRef<Map<string, number>>(new Map());
	const bookmarkHighlightTimers = useRef<Map<string, number>>(new Map());

	useEffect(() => {
		const unsubscribe = window.how.onStatus((nextStatus) => {
			setStatus(nextStatus);
		});
		return () => {
			unsubscribe();
		};
	}, [setStatus]);

	useEffect(() => {
		const previous = previousCheckpointTitles.current;
		const next = new Map(
			status.checkpoints.map((checkpoint) => [checkpointTimelineKey(checkpoint), checkpoint.title]),
		);
		if (previous) {
			const changedCheckpointKeys = status.checkpoints
				.filter((checkpoint) => {
					const previousTitle = previous.get(checkpointTimelineKey(checkpoint));
					return previousTitle !== undefined && previousTitle !== checkpoint.title;
				})
				.map(checkpointTimelineKey);

			if (changedCheckpointKeys.length > 0) {
				setHighlightedCheckpointKeys((current) => {
					const updated = new Set(current);
					for (const checkpointKey of changedCheckpointKeys) updated.add(checkpointKey);
					return updated;
				});
				for (const checkpointKey of changedCheckpointKeys) {
					const existingTimer = checkpointHighlightTimers.current.get(checkpointKey);
					if (existingTimer !== undefined) window.clearTimeout(existingTimer);
					const timer = window.setTimeout(() => {
						setHighlightedCheckpointKeys((current) => {
							const updated = new Set(current);
							updated.delete(checkpointKey);
							return updated;
						});
						checkpointHighlightTimers.current.delete(checkpointKey);
					}, 1200);
					checkpointHighlightTimers.current.set(checkpointKey, timer);
				}
			}
		}
		previousCheckpointTitles.current = next;
	}, [status.checkpoints]);

	useEffect(() => {
		const timers = checkpointHighlightTimers.current;
		return () => {
			for (const timer of timers.values()) window.clearTimeout(timer);
			timers.clear();
		};
	}, []);

	useEffect(() => {
		const previous = previousBookmarkIds.current;
		const previousProjectId = previousBookmarkProjectId.current;
		const currentProjectId = status.project?.id ?? null;
		const next = new Set(status.bookmarks.map((bookmark) => bookmark.id));
		if (previous && previousProjectId === currentProjectId) {
			const createdBookmarkIds = status.bookmarks
				.filter((bookmark) => bookmark.kind === "user" && !previous.has(bookmark.id))
				.map((bookmark) => bookmark.id);

			if (createdBookmarkIds.length > 0) {
				setHighlightedBookmarkIds((current) => {
					const updated = new Set(current);
					for (const bookmarkId of createdBookmarkIds) updated.add(bookmarkId);
					return updated;
				});
				for (const bookmarkId of createdBookmarkIds) {
					const existingTimer = bookmarkHighlightTimers.current.get(bookmarkId);
					if (existingTimer !== undefined) window.clearTimeout(existingTimer);
					const timer = window.setTimeout(() => {
						setHighlightedBookmarkIds((current) => {
							const updated = new Set(current);
							updated.delete(bookmarkId);
							return updated;
						});
						bookmarkHighlightTimers.current.delete(bookmarkId);
					}, 1200);
					bookmarkHighlightTimers.current.set(bookmarkId, timer);
				}
			}
		}
		previousBookmarkIds.current = next;
		previousBookmarkProjectId.current = currentProjectId;
	}, [status.bookmarks, status.project?.id]);

	useEffect(() => {
		const timers = bookmarkHighlightTimers.current;
		return () => {
			for (const timer of timers.values()) window.clearTimeout(timer);
			timers.clear();
		};
	}, []);

	async function runPending<T>(action: PendingAction, work: () => Promise<T>): Promise<T> {
		setPendingAction(action);
		try {
			return await work();
		} finally {
			setPendingAction(null);
		}
	}

	async function runProjectAction(
		pending: PendingAction,
		action: () => Promise<{ type: "cancelled" } | { type: "opened"; status: HowStatus }>,
	) {
		setError(null);
		try {
			const result = await runPending(pending, action);
			if (result.type === "opened") setStatus(result.status);
		} catch {
			setError("How could not open that project.");
		}
	}

	async function leaveCleanBrowsing(): Promise<boolean> {
		if (!status.browsing) {
			return true;
		}
		if (status.browsing.dirty) {
			return false;
		}
		setError(null);
		try {
			setStatus(await runPending("returnToLatest", async () => await window.how.returnToLatest()));
			return true;
		} catch {
			setError("How could not return to latest.");
			return false;
		}
	}

	async function openProject() {
		if (status.browsing?.dirty) {
			setPendingDirtyAction({ type: "open" });
			return;
		}
		if (!(await leaveCleanBrowsing())) return;
		await runProjectAction("openProject", async () => await window.how.openProject());
	}

	async function startProject() {
		if (status.browsing?.dirty) {
			setPendingDirtyAction({ type: "start" });
			return;
		}
		if (!(await leaveCleanBrowsing())) return;
		await runProjectAction("startProject", async () => await window.how.startProject());
	}

	async function deleteProject() {
		if (status.browsing?.dirty) {
			setPendingDirtyAction({ type: "delete" });
			return;
		}
		if (!(await leaveCleanBrowsing())) return;
		const confirmed = window.confirm(
			"Remove this project from How? Your project folder and files will stay where they are.",
		);
		if (!confirmed) return;
		setError(null);
		try {
			setStatus(await runPending("deleteProject", async () => await window.how.deleteProject()));
		} catch {
			setError("How could not delete that project.");
		}
	}

	async function viewCheckpoint(checkpoint: Checkpoint) {
		if (status.browsing?.dirty) {
			setPendingDirtyAction({ type: "view", checkpoint });
			return;
		}
		setError(null);
		try {
			setStatus(
				await runPending(
					"viewCheckpoint",
					async () => await window.how.viewCheckpoint(checkpoint.id),
				),
			);
		} catch {
			setError("How could not view that checkpoint.");
		}
	}

	async function createBookmark() {
		if (status.browsing?.dirty) {
			setError("Continue from here before bookmarking these changes.");
			return;
		}
		setBookmarkNameAction({ type: "create" });
	}

	async function saveBookmarkName(name: string) {
		const action = bookmarkNameAction;
		if (!action || name.trim().length === 0) return;
		setError(null);
		try {
			if (action.type === "create" && status.browsing) {
				const checkpointId = status.browsing.currentCheckpointId;
				setStatus(
					await runPending(
						"createBookmark",
						async () => await window.how.createBookmarkFromCheckpoint(name, checkpointId),
					),
				);
			} else if (action.type === "create") {
				setStatus(
					await runPending("createBookmark", async () => await window.how.createBookmark(name)),
				);
			} else {
				setStatus(
					await runPending(
						"renameBookmark",
						async () => await window.how.renameBookmark(action.bookmark.id, name),
					),
				);
			}
			setBookmarkNameAction(null);
		} catch {
			setError(
				action.type === "create"
					? "How could not create that bookmark."
					: "How could not rename that bookmark.",
			);
		}
	}

	async function switchBookmark(bookmark: Bookmark) {
		if (bookmark.isCurrent) return;
		if (status.browsing?.dirty) {
			setPendingDirtyAction({ type: "switchBookmark", bookmark });
			return;
		}
		setError(null);
		try {
			setStatus(
				await runPending(
					"switchBookmark",
					async () => await window.how.switchBookmark(bookmark.id),
				),
			);
		} catch {
			setError("How could not switch bookmarks.");
		}
	}

	async function updateBookmark(bookmark: Bookmark) {
		setBookmarkConfirmAction({ type: "update", bookmark });
	}

	async function confirmBookmarkAction() {
		const action = bookmarkConfirmAction;
		if (!action) return;
		setError(null);
		try {
			if (action.type === "update")
				setStatus(
					await runPending(
						"updateBookmark",
						async () => await window.how.updateBookmark(action.bookmark.id),
					),
				);
			else
				setStatus(
					await runPending(
						"deleteBookmark",
						async () => await window.how.deleteBookmark(action.bookmark.id),
					),
				);
			setBookmarkConfirmAction(null);
		} catch {
			setError(
				action.type === "update"
					? "How could not update that bookmark."
					: "How could not delete that bookmark.",
			);
		}
	}

	async function renameBookmark(bookmark: Bookmark) {
		setBookmarkNameAction({ type: "rename", bookmark });
	}

	async function deleteBookmark(bookmark: Bookmark) {
		setBookmarkConfirmAction({ type: "delete", bookmark });
	}

	async function continueFromCheckpoint() {
		setError(null);
		try {
			setStatus(
				await runPending(
					"continueFromCheckpoint",
					async () => await window.how.continueFromCheckpoint(),
				),
			);
		} catch {
			setError("How could not continue from here.");
		}
	}

	async function returnToLatest() {
		if (status.browsing?.dirty) {
			setPendingDirtyAction({ type: "returnToLatest" });
			return;
		}
		setError(null);
		try {
			setStatus(await runPending("returnToLatest", async () => await window.how.returnToLatest()));
		} catch {
			setError("How could not return to latest.");
		}
	}

	async function handlePublishResult(
		result: Awaited<ReturnType<typeof window.how.publishProject>>,
	): Promise<void> {
		setStatus(result.status);
		if (result.type === "needsGithubLogin") setShowGithubLoginDialog(true);
		if (result.type === "needsGithubRepository") {
			setGithubRepositoryName(result.defaultRepositoryName);
			if (result.repositories) setGithubRepositories(result.repositories);
			setShowGithubRepositoryChoiceDialog(true);
		}
	}

	async function publishProject() {
		setError(null);
		try {
			await handlePublishResult(
				await runPending("publish", async () => await window.how.publishProject()),
			);
		} catch {
			setError("How could not publish to the shared project.");
		}
	}

	async function loginToGithub() {
		setError(null);
		try {
			const result = await runPending("githubLogin", async () => await window.how.loginToGithub());
			if (result.type === "failed") {
				setError(result.message);
				return;
			}
			setShowGithubLoginDialog(false);
			await handlePublishResult(
				await runPending("publish", async () => await window.how.publishProject()),
			);
		} catch {
			setError("How could not log in to GitHub.");
		}
	}

	async function createGithubRepository(name: string) {
		setError(null);
		try {
			setShowCreateGithubRepositoryDialog(false);
			await handlePublishResult(
				await runPending(
					"githubCreateRepository",
					async () => await window.how.publishProject({ createGithubRepositoryName: name.trim() }),
				),
			);
		} catch {
			setError("How could not publish to the shared project.");
		}
	}

	async function loadGithubRepositories() {
		setError(null);
		try {
			const result = await runPending(
				"githubLoadRepositories",
				async () => await window.how.listGithubRepositories(),
			);
			if (result.type === "failed") {
				setError(result.message);
				return;
			}
			setGithubRepositories(result.repositories);
			setShowGithubRepositoryChoiceDialog(false);
			setShowChooseGithubRepositoryDialog(true);
		} catch {
			setError("How could not load GitHub projects.");
		}
	}

	async function publishWithGithubRepository(repository: GithubRepositorySummary) {
		setError(null);
		try {
			setShowChooseGithubRepositoryDialog(false);
			await handlePublishResult(
				await runPending(
					"githubPublishRepository",
					async () =>
						await window.how.publishProject({ githubRepositoryCloneUrl: repository.cloneUrl }),
				),
			);
		} catch {
			setError("How could not publish to the shared project.");
		}
	}

	async function leaveBrowsingChanges() {
		const action = pendingDirtyAction;
		if (!action) return;
		setPendingDirtyAction(null);
		setError(null);
		try {
			if (action.type === "view")
				setStatus(
					await runPending(
						"leaveBrowsingChanges",
						async () =>
							await window.how.viewCheckpoint(action.checkpoint.id, {
								discardBrowsingChanges: true,
							}),
					),
				);
			if (action.type === "switchBookmark") {
				await runPending("leaveBrowsingChanges", async () => {
					setStatus(await window.how.returnToLatest({ discardBrowsingChanges: true }));
					setStatus(await window.how.switchBookmark(action.bookmark.id));
				});
			}
			if (action.type === "returnToLatest")
				setStatus(
					await runPending(
						"leaveBrowsingChanges",
						async () => await window.how.returnToLatest({ discardBrowsingChanges: true }),
					),
				);
			if (action.type === "open") {
				await runPending("leaveBrowsingChanges", async () => {
					setStatus(await window.how.returnToLatest({ discardBrowsingChanges: true }));
				});
				await runProjectAction("openProject", async () => await window.how.openProject());
				return;
			}
			if (action.type === "start") {
				await runPending("leaveBrowsingChanges", async () => {
					setStatus(await window.how.returnToLatest({ discardBrowsingChanges: true }));
				});
				await runProjectAction("startProject", async () => await window.how.startProject());
				return;
			}
			if (action.type === "delete") {
				await runPending("leaveBrowsingChanges", async () => {
					setStatus(await window.how.returnToLatest({ discardBrowsingChanges: true }));
				});
				const confirmed = window.confirm(
					"Remove this project from How? Your project folder and files will stay where they are.",
				);
				if (confirmed)
					setStatus(
						await runPending("deleteProject", async () => await window.how.deleteProject()),
					);
				return;
			}
		} catch {
			setError("How could not leave those changes.");
		}
	}

	const projectPickerBusy = pendingAction === "openProject" || pendingAction === "startProject";
	const bookmarkDialogBusy =
		pendingAction === "createBookmark" ||
		pendingAction === "renameBookmark" ||
		pendingAction === "updateBookmark" ||
		pendingAction === "deleteBookmark";
	const githubDialogBusy =
		pendingAction === "publish" ||
		pendingAction === "githubLogin" ||
		pendingAction === "githubCreateRepository" ||
		pendingAction === "githubLoadRepositories" ||
		pendingAction === "githubPublishRepository";

	if (!status.project)
		return (
			<>
				<EmptyState
					onOpen={openProject}
					onStart={startProject}
					busy={projectPickerBusy}
					error={error}
				/>
				{pendingDirtyAction ? (
					<DirtyBrowsingDialog
						onLeave={leaveBrowsingChanges}
						onCancel={() => setPendingDirtyAction(null)}
					/>
				) : null}
			</>
		);

	return (
		<>
			<ProjectScreen
				status={status}
				highlightedCheckpointKeys={highlightedCheckpointKeys}
				highlightedBookmarkIds={highlightedBookmarkIds}
				pendingAction={pendingAction}
				onOpen={openProject}
				onStart={startProject}
				onDelete={deleteProject}
				onPublish={publishProject}
				onCreateBookmark={createBookmark}
				onSwitchBookmark={switchBookmark}
				onUpdateBookmark={updateBookmark}
				onRenameBookmark={renameBookmark}
				onDeleteBookmark={deleteBookmark}
				onView={viewCheckpoint}
				onContinue={continueFromCheckpoint}
				onReturnToLatest={returnToLatest}
			/>
			{pendingDirtyAction ? (
				<DirtyBrowsingDialog
					onLeave={leaveBrowsingChanges}
					onCancel={() => setPendingDirtyAction(null)}
				/>
			) : null}
			{bookmarkNameAction ? (
				<BookmarkNameDialog
					title={
						bookmarkNameAction.type === "create" ? "Bookmark current state" : "Rename bookmark"
					}
					initialName={bookmarkNameAction.type === "rename" ? bookmarkNameAction.bookmark.name : ""}
					onSave={saveBookmarkName}
					onCancel={() => setBookmarkNameAction(null)}
					busy={bookmarkDialogBusy}
				/>
			) : null}
			{bookmarkConfirmAction ? (
				<BookmarkConfirmDialog
					title={bookmarkConfirmAction.type === "update" ? "Update bookmark?" : "Delete bookmark?"}
					body={
						bookmarkConfirmAction.type === "update"
							? `Replace "${bookmarkConfirmAction.bookmark.name}" with where you are now?`
							: `Delete "${bookmarkConfirmAction.bookmark.name}"? Your files will stay unchanged.`
					}
					action={bookmarkConfirmAction.type === "update" ? "Update" : "Delete"}
					onConfirm={confirmBookmarkAction}
					onCancel={() => setBookmarkConfirmAction(null)}
					busy={bookmarkDialogBusy}
				/>
			) : null}
			{showGithubLoginDialog ? (
				<GithubLoginDialog
					onLogin={loginToGithub}
					onCancel={() => setShowGithubLoginDialog(false)}
					busy={githubDialogBusy}
				/>
			) : null}
			{showGithubRepositoryChoiceDialog ? (
				<GithubRepositoryChoiceDialog
					onCreate={() => {
						setShowGithubRepositoryChoiceDialog(false);
						setShowCreateGithubRepositoryDialog(true);
					}}
					onChoose={loadGithubRepositories}
					onCancel={() => setShowGithubRepositoryChoiceDialog(false)}
					busy={githubDialogBusy}
				/>
			) : null}
			{showCreateGithubRepositoryDialog ? (
				<CreateGithubRepositoryDialog
					defaultName={githubRepositoryName}
					onPublish={createGithubRepository}
					onCancel={() => setShowCreateGithubRepositoryDialog(false)}
					busy={githubDialogBusy}
				/>
			) : null}
			{showChooseGithubRepositoryDialog ? (
				<ChooseGithubRepositoryDialog
					repositories={githubRepositories}
					onPublish={publishWithGithubRepository}
					onCancel={() => setShowChooseGithubRepositoryDialog(false)}
					busy={githubDialogBusy}
				/>
			) : null}
		</>
	);
}

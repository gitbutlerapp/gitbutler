import { Button } from "#ui/components/ui/button.tsx";
import {
	AlertCircle,
	Check,
	Clock,
	Eye,
	FolderOpen,
	Plus,
	RefreshCw,
	Trash2,
} from "lucide-react";
import { useEffect, useState } from "react";
import type { BrowsingSession, Checkpoint, HowStatus, SaveState } from "../../../electron/src/ipc";

const initialStatus: HowStatus = {
	project: null,
	saveState: "idle",
	message: null,
	lastSavedAt: null,
	checkpoints: [],
	browsing: null,
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
	| { type: "returnToLatest" }
	| { type: "open" }
	| { type: "start" }
	| { type: "delete" };

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
					You changed files while browsing this checkpoint. To keep those changes, choose
					Continue from here first.
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

function Timeline({
	checkpoints,
	browsing,
	onView,
	busy,
}: {
	checkpoints: Array<Checkpoint>;
	browsing: BrowsingSession | null;
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
					key={checkpoint.id}
					className={`group grid grid-cols-[auto_1fr_auto] gap-4 rounded-md border px-4 py-3 ${
						browsing?.currentCheckpointId === checkpoint.id
							? "border-stone-500 bg-white"
							: "border-stone-200 bg-stone-100"
					}`}
				>
					<div className="mt-1 h-2.5 w-2.5 rounded-full bg-stone-700" />
					<div className="min-w-0 flex-1">
						<div className="flex min-w-0 items-center gap-2">
							<p className="truncate text-sm font-medium text-stone-950">{checkpoint.title}</p>
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

function ProjectScreen({
	status,
	onOpen,
	onStart,
	onDelete,
	onView,
	onContinue,
	onReturnToLatest,
	busy,
}: {
	status: HowStatus;
	onOpen: () => Promise<void>;
	onStart: () => Promise<void>;
	onDelete: () => Promise<void>;
	onView: (checkpoint: Checkpoint) => Promise<void>;
	onContinue: () => Promise<void>;
	onReturnToLatest: () => Promise<void>;
	busy: boolean;
}) {
	const project = status.project;
	if (!project) return null;

	return (
		<main className="min-h-screen px-6 py-6">
			<div className="mx-auto flex w-full h-full max-w-5xl flex-col justify-start gap-8">
				<nav>
					<Button variant="ghost" size="sm" onClick={() => void onOpen()} disabled={busy}>
						<FolderOpen className="h-4 w-4" aria-hidden />
						Open
					</Button>
					<Button variant="ghost" size="sm" onClick={() => void onStart()} disabled={busy}>
						<Plus className="h-4 w-4" aria-hidden />
						Start
					</Button>
					<Button variant="ghost" size="sm" onClick={() => void onDelete()} disabled={busy}>
						<Trash2 className="h-4 w-4" aria-hidden />
						Delete
					</Button>
				</nav>
				<header className="flex flex-wrap items-start justify-between gap-4 pb-5">
					<div className="min-w-0 flex-1">
						<h1 className="truncate text-xl font-semibold tracking-normal text-stone-700">
							{project.title}
						</h1>
					</div>
					<div className="flex items-center gap-2">
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
					<section className="flex flex-wrap items-center justify-between gap-3 rounded-md border border-stone-200 bg-white px-4 py-3">
						<p className="text-sm text-stone-600">You are viewing an earlier checkpoint.</p>
						<div className="flex flex-wrap gap-2">
							<Button variant="secondary" onClick={() => void onReturnToLatest()} disabled={busy}>
								Return to latest
							</Button>
							<Button onClick={() => void onContinue()} disabled={busy}>
								Continue from here
							</Button>
						</div>
					</section>
				) : null}

				<section>
					<Timeline
						checkpoints={status.checkpoints}
						browsing={status.browsing}
						onView={onView}
						busy={busy}
					/>
				</section>
			</div>
		</main>
	);
}

export function HowHome() {
	const [status, setStatus] = useState<HowStatus>(initialStatus);
	const [busy, setBusy] = useState(false);
	const [error, setError] = useState<string | null>(null);
	const [pendingDirtyAction, setPendingDirtyAction] = useState<PendingDirtyAction | null>(null);

	useEffect(() => {
		let mounted = true;
		void window.how.getStatus().then((nextStatus) => {
			if (mounted) setStatus(nextStatus);
		});
		const unsubscribe = window.how.onStatus((nextStatus) => {
			setStatus(nextStatus);
		});
		return () => {
			mounted = false;
			unsubscribe();
		};
	}, []);

	async function runProjectAction(
		action: () => Promise<{ type: "cancelled" } | { type: "opened"; status: HowStatus }>,
	) {
		setBusy(true);
		setError(null);
		try {
			const result = await action();
			if (result.type === "opened") setStatus(result.status);
		} catch {
			setError("How could not open that project.");
		} finally {
			setBusy(false);
		}
	}

	async function leaveCleanBrowsing(): Promise<boolean> {
		if (!status.browsing) {
			return true;
		}
		if (status.browsing.dirty) {
			return false;
		}
		setBusy(true);
		setError(null);
		try {
			setStatus(await window.how.returnToLatest());
			return true;
		} catch {
			setError("How could not return to latest.");
			return false;
		} finally {
			setBusy(false);
		}
	}

	async function openProject() {
		if (status.browsing?.dirty) {
			setPendingDirtyAction({ type: "open" });
			return;
		}
		if (!(await leaveCleanBrowsing())) return;
		await runProjectAction(async () => await window.how.openProject());
	}

	async function startProject() {
		if (status.browsing?.dirty) {
			setPendingDirtyAction({ type: "start" });
			return;
		}
		if (!(await leaveCleanBrowsing())) return;
		await runProjectAction(async () => await window.how.startProject());
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
		setBusy(true);
		setError(null);
		try {
			setStatus(await window.how.deleteProject());
		} catch {
			setError("How could not delete that project.");
		} finally {
			setBusy(false);
		}
	}

	async function viewCheckpoint(checkpoint: Checkpoint) {
		if (status.browsing?.dirty) {
			setPendingDirtyAction({ type: "view", checkpoint });
			return;
		}
		setBusy(true);
		setError(null);
		try {
			setStatus(await window.how.viewCheckpoint(checkpoint.id));
		} catch {
			setError("How could not view that checkpoint.");
		} finally {
			setBusy(false);
		}
	}

	async function continueFromCheckpoint() {
		setBusy(true);
		setError(null);
		try {
			setStatus(await window.how.continueFromCheckpoint());
		} catch {
			setError("How could not continue from here.");
		} finally {
			setBusy(false);
		}
	}

	async function returnToLatest() {
		if (status.browsing?.dirty) {
			setPendingDirtyAction({ type: "returnToLatest" });
			return;
		}
		setBusy(true);
		setError(null);
		try {
			setStatus(await window.how.returnToLatest());
		} catch {
			setError("How could not return to latest.");
		} finally {
			setBusy(false);
		}
	}

	async function leaveBrowsingChanges() {
		const action = pendingDirtyAction;
		if (!action) return;
		setPendingDirtyAction(null);
		setBusy(true);
		setError(null);
		try {
			if (action.type === "view")
				setStatus(
					await window.how.viewCheckpoint(action.checkpoint.id, {
						discardBrowsingChanges: true,
					}),
				);
			if (action.type === "returnToLatest")
				setStatus(await window.how.returnToLatest({ discardBrowsingChanges: true }));
			if (action.type === "open") {
				setStatus(await window.how.returnToLatest({ discardBrowsingChanges: true }));
				setBusy(false);
				await runProjectAction(async () => await window.how.openProject());
				return;
			}
			if (action.type === "start") {
				setStatus(await window.how.returnToLatest({ discardBrowsingChanges: true }));
				setBusy(false);
				await runProjectAction(async () => await window.how.startProject());
				return;
			}
			if (action.type === "delete") {
				setStatus(await window.how.returnToLatest({ discardBrowsingChanges: true }));
				setBusy(false);
				const confirmed = window.confirm(
					"Remove this project from How? Your project folder and files will stay where they are.",
				);
				if (confirmed) setStatus(await window.how.deleteProject());
				return;
			}
		} catch {
			setError("How could not leave those changes.");
		} finally {
			setBusy(false);
		}
	}

	if (!status.project)
		return (
			<>
				<EmptyState onOpen={openProject} onStart={startProject} busy={busy} error={error} />
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
				onOpen={openProject}
				onStart={startProject}
				onDelete={deleteProject}
				onView={viewCheckpoint}
				onContinue={continueFromCheckpoint}
				onReturnToLatest={returnToLatest}
				busy={busy}
			/>
			{pendingDirtyAction ? (
				<DirtyBrowsingDialog
					onLeave={leaveBrowsingChanges}
					onCancel={() => setPendingDirtyAction(null)}
				/>
			) : null}
		</>
	);
}

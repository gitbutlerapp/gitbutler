import { Button } from "#ui/components/ui/button.tsx";
import { AlertCircle, Check, Clock, FolderOpen, Plus, RefreshCw, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import type { Checkpoint, HowStatus, SaveState } from "../../../electron/src/ipc";

const initialStatus: HowStatus = {
	project: null,
	saveState: "idle",
	message: null,
	lastSavedAt: null,
	checkpoints: [],
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

function Timeline({ checkpoints }: { checkpoints: Array<Checkpoint> }) {
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
			{checkpoints.map((checkpoint) => (
				<li
					key={checkpoint.id}
					className="grid grid-cols-[auto_1fr] gap-4 rounded-md bg-stone-100 border border-stone-200 px-4 py-3"
				>
					<div className="mt-1 h-2.5 w-2.5 rounded-full bg-stone-700" />
					<div className="min-w-0 flex-1">
						<p className="truncate text-sm font-medium text-stone-950">{checkpoint.title}</p>
						<p className="mt-1 text-xs text-stone-500">{formatTime(checkpoint.createdAt)}</p>
					</div>
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
	busy,
}: {
	status: HowStatus;
	onOpen: () => Promise<void>;
	onStart: () => Promise<void>;
	onDelete: () => Promise<void>;
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

				<section>
					<Timeline checkpoints={status.checkpoints} />
				</section>
			</div>
		</main>
	);
}

export function HowHome() {
	const [status, setStatus] = useState<HowStatus>(initialStatus);
	const [busy, setBusy] = useState(false);
	const [error, setError] = useState<string | null>(null);

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

	async function openProject() {
		await runProjectAction(async () => await window.how.openProject());
	}

	async function startProject() {
		await runProjectAction(async () => await window.how.startProject());
	}

	async function deleteProject() {
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

	if (!status.project)
		return <EmptyState onOpen={openProject} onStart={startProject} busy={busy} error={error} />;

	return (
		<ProjectScreen
			status={status}
			onOpen={openProject}
			onStart={startProject}
			onDelete={deleteProject}
			busy={busy}
		/>
	);
}

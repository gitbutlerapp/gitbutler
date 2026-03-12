import {
	longRunningCancelTsfn,
	LongRunningEventKind,
	longRunningStartTsfn,
} from "@gitbutler/but-sdk";
import type { LongRunningTaskSnapshot } from "#electron/ipc";

const MAX_DURATION_MS = 600000;
type LongRunningTaskListener = (event: LongRunningTaskSnapshot) => void;

const activeTaskIds = new Set<number>();
const tasks = new Map<number, LongRunningTaskSnapshot>();
const listeners = new Set<LongRunningTaskListener>();

/**
	* Starts a non-blocking task in Rust and tracks task snapshots for renderer consumption.
 */
export function startLongRunningTask(durationMs: number): number {
	if (!Number.isInteger(durationMs) || durationMs < 1 || durationMs > MAX_DURATION_MS) {
		throw new Error("durationMs must be an integer between 1 and 600000.");
	}

	let taskId = 0;

	taskId = longRunningStartTsfn(durationMs, (err, event) => {
		const currentSnapshot = tasks.get(taskId);
		if (!currentSnapshot) {
			return;
		}

		if (err) {
			activeTaskIds.delete(taskId);
			const nextSnapshot: LongRunningTaskSnapshot = {
				...currentSnapshot,
				status: "error",
				message: err.message ?? "unknown error",
			};
			setTaskSnapshot(nextSnapshot);
			return;
		}

		if (!activeTaskIds.has(taskId)) {
			return;
		}

		const snapshotWithStep: LongRunningTaskSnapshot = {
			...currentSnapshot,
			step: typeof event.step === "number" ? event.step : currentSnapshot.step,
		};

		if (event.kind === LongRunningEventKind.Progress) {
			setTaskSnapshot({
				...snapshotWithStep,
				status: "running",
				message: undefined,
			});
			return;
		}

		if (event.kind === LongRunningEventKind.Done) {
			activeTaskIds.delete(taskId);
			setTaskSnapshot({
				...snapshotWithStep,
				status: "done",
				message: undefined,
			});
			return;
		}

		if (event.kind === LongRunningEventKind.Cancelled) {
			activeTaskIds.delete(taskId);
			setTaskSnapshot({
				...snapshotWithStep,
				status: "cancelled",
				message: undefined,
			});
			return;
		}

		activeTaskIds.delete(taskId);
		setTaskSnapshot({
			...snapshotWithStep,
			status: "error",
			message: event.message ?? "unknown error",
		});
	});

	activeTaskIds.add(taskId);
	const initialSnapshot: LongRunningTaskSnapshot = {
		taskId,
		durationMs,
		step: 0,
		status: "running",
	};
	setTaskSnapshot(initialSnapshot);

	return taskId;
}

/**
 * Requests cancellation for a task and marks it as cancelling until the Rust callback emits a terminal state.
 */
export function cancelLongRunningTask(taskId: number): boolean {
	if (!activeTaskIds.has(taskId)) {
		return false;
	}

	const snapshot = tasks.get(taskId);
	if (snapshot) {
		const nextSnapshot: LongRunningTaskSnapshot = {
			...snapshot,
			status: "cancelling",
			message: undefined,
		};
		setTaskSnapshot(nextSnapshot);
	}

	const cancelled = longRunningCancelTsfn(taskId);
	if (!cancelled && snapshot) {
		const nextSnapshot: LongRunningTaskSnapshot = {
			...snapshot,
			status: "error",
			message: "Task could not be cancelled (already finished).",
		};
		setTaskSnapshot(nextSnapshot);
	}

	return cancelled;
}

/**
	* Returns all known task snapshots, including terminal ones, newest first.
 */
export function listLongRunningTasks(): LongRunningTaskSnapshot[] {
	return [...tasks.values()].sort((left, right) => right.taskId - left.taskId);
}

export function subscribeLongRunningTaskEvents(listener: LongRunningTaskListener): () => void {
	listeners.add(listener);

	return () => {
		listeners.delete(listener);
	};
}

function setTaskSnapshot(snapshot: LongRunningTaskSnapshot): void {
	tasks.set(snapshot.taskId, snapshot);
	emitLongRunningTaskEvent(snapshot);
}

function emitLongRunningTaskEvent(event: LongRunningTaskSnapshot): void {
	for (const listener of listeners) {
		listener(event);
	}
}

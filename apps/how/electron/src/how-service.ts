import { checkpointMessage, createDiffSpec } from "./checkpoints.js";
import {
	createInitialCheckpointCommit,
	discoverGitDir,
	encodeProjectHandle,
	ensureGitRepository,
	listCheckpointCommits,
	projectTitleFromPath,
	worktreeFromGitDir,
} from "./git.js";
import { howIpcChannels, type Checkpoint, type HowStatus, type ProjectSummary } from "./ipc.js";
import { plainErrorMessage } from "./plain-error.js";
import {
	changesInWorktree,
	commitCreate,
	headInfo,
	type InsertSide,
	type RelativeTo,
	watcherStart,
	type WatcherHandle,
	type WatcherEvent,
} from "@gitbutler/but-sdk";
import fs from "node:fs/promises";
import path from "node:path";
import type { BrowserWindow } from "electron";

const checkpointLimit = 50;
const checkpointQuietPeriodMs = 10_000;

type StoredState = {
	activeProject: ProjectSummary | null;
};

const emptyStoredState: StoredState = {
	activeProject: null,
};

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null;
}

export class HowService {
	#status: HowStatus = {
		project: null,
		saveState: "idle",
		message: null,
		lastSavedAt: null,
		checkpoints: [],
	};
	#watcher: WatcherHandle | null = null;
	#debounce: NodeJS.Timeout | null = null;
	#saving = false;
	#dirtyWhileSaving = false;

	constructor(
		private readonly statePath: string,
		private readonly getWindows: () => Array<BrowserWindow>,
	) {}

	async initialize(): Promise<HowStatus> {
		const stored = await this.#readStoredState();
		if (stored.activeProject) {
			this.#status = {
				...this.#status,
				project: stored.activeProject,
				saveState: "watching",
				message: "Watching for changes",
			};
			await this.#refreshTimeline();
			await this.#startWatching(stored.activeProject);
		}
		this.#emit();
		return this.getStatus();
	}

	getStatus(): HowStatus {
		return this.#status;
	}

	async openProjectFromPath(selectedPath: string): Promise<HowStatus> {
		const gitDir = await discoverGitDir(selectedPath);
		const worktreePath = await worktreeFromGitDir(gitDir);
		return await this.#activateProject({ gitDir, worktreePath });
	}

	async startProjectAtPath(selectedPath: string): Promise<HowStatus> {
		const gitDir = await ensureGitRepository(selectedPath);
		const worktreePath = await worktreeFromGitDir(gitDir);
		return await this.#activateProject({ gitDir, worktreePath });
	}

	async createCheckpointNow(): Promise<HowStatus> {
		await this.#createCheckpoint();
		return this.getStatus();
	}

	async stop(): Promise<void> {
		if (this.#debounce) clearTimeout(this.#debounce);
		this.#debounce = null;
		if (this.#watcher) this.#watcher.stop();
		this.#watcher = null;
	}

	async #activateProject({
		gitDir,
		worktreePath,
	}: {
		gitDir: string;
		worktreePath: string;
	}): Promise<HowStatus> {
		const project: ProjectSummary = {
			id: encodeProjectHandle(gitDir),
			title: projectTitleFromPath(worktreePath),
			path: worktreePath,
			gitDir,
		};

		await this.stop();
		this.#status = {
			project,
			saveState: "watching",
			message: "Watching for changes",
			lastSavedAt: null,
			checkpoints: [],
		};
		await this.#writeStoredState({ activeProject: project });
		await this.#refreshTimeline();
		await this.#startWatching(project);
		this.#emit();
		return this.getStatus();
	}

	async #startWatching(project: ProjectSummary): Promise<void> {
		this.#watcher = await watcherStart(project.id, (err: Error | null, event: WatcherEvent) => {
			if (err) {
				this.#setError(err);
				return;
			}
			if (this.#eventShouldScheduleCheckpoint(event)) this.#scheduleCheckpoint();
		});
	}

	#eventShouldScheduleCheckpoint(event: WatcherEvent): boolean {
		return (
			event.name.endsWith("/worktree_changes") ||
			event.name.endsWith("/workspace-activity") ||
			event.name.endsWith("/git/head")
		);
	}

	#scheduleCheckpoint(): void {
		if (!this.#status.project) return;
		if (this.#debounce) clearTimeout(this.#debounce);
		this.#status = {
			...this.#status,
			saveState: "pending",
			message: "Saving soon",
		};
		this.#emit();
		this.#debounce = setTimeout(() => {
			this.#debounce = null;
			void this.#createCheckpoint();
		}, checkpointQuietPeriodMs);
	}

	async #createCheckpoint(): Promise<void> {
		const project = this.#status.project;
		if (!project) return;

		if (this.#saving) {
			this.#dirtyWhileSaving = true;
			return;
		}

		this.#saving = true;
		this.#status = {
			...this.#status,
			saveState: "saving",
			message: "Saving",
		};
		this.#emit();

		try {
			const worktreeChanges = await changesInWorktree(project.id);
			if (worktreeChanges.changes.length === 0) {
				this.#status = {
					...this.#status,
					saveState: "watching",
					message: "Watching for changes",
				};
				this.#emit();
				return;
			}

			const message = checkpointMessage(new Date());
			const changes = worktreeChanges.changes.map((change) => createDiffSpec(change));
			const placement = await this.#checkpointPlacement(project.id);

			if (placement) {
				const result = await commitCreate(
					project.id,
					placement.relativeTo,
					placement.side,
					changes,
					message,
					false,
				);
				if (result.rejectedChanges.length > 0 || result.newCommit === null)
					throw new Error("Some project changes could not be saved.");
			} else {
				const commitId = await createInitialCheckpointCommit(project.path, message);
				if (commitId === null) return;
			}

			await this.#refreshTimeline();
			this.#status = {
				...this.#status,
				saveState: "saved",
				message: "Saved just now",
				lastSavedAt: Date.now(),
			};
			this.#emit();
		} catch (error) {
			this.#setError(error);
		} finally {
			this.#saving = false;
			if (this.#dirtyWhileSaving) {
				this.#dirtyWhileSaving = false;
				this.#scheduleCheckpoint();
			}
		}
	}

	async #checkpointPlacement(
		projectId: string,
	): Promise<{ relativeTo: RelativeTo; side: InsertSide } | null> {
		const info = await headInfo(projectId);
		if (info.workspaceRef) {
			return {
				relativeTo: { type: "referenceBytes", subject: info.workspaceRef.fullNameBytes },
				side: "below",
			};
		}

		const entrypointSegment =
			info.stacks.flatMap((stack) => stack.segments).find((segment) => segment.isEntrypoint) ??
			info.stacks.flatMap((stack) => stack.segments)[0];
		const topCommit = entrypointSegment?.commits[0];
		if (!topCommit) return null;

		return {
			relativeTo: { type: "commit", subject: topCommit.id },
			side: "above",
		};
	}

	async #refreshTimeline(): Promise<void> {
		const project = this.#status.project;
		if (!project) return;

		const commits = await listCheckpointCommits(project.path, checkpointLimit);
		const checkpoints: Array<Checkpoint> = commits.map((commit) => ({
			id: commit.id,
			title: commit.title,
			createdAt: commit.createdAt,
		}));
		this.#status = {
			...this.#status,
			checkpoints,
		};
	}

	#setError(error: unknown): void {
		this.#status = {
			...this.#status,
			saveState: "error",
			message: plainErrorMessage(error),
		};
		this.#emit();
	}

	#emit(): void {
		for (const window of this.getWindows())
			window.webContents.send(howIpcChannels.status, this.#status);
	}

	async #readStoredState(): Promise<StoredState> {
		try {
			const raw = await fs.readFile(this.statePath, "utf8");
			const parsed: unknown = JSON.parse(raw);
			if (!isRecord(parsed)) return emptyStoredState;
			const activeProject = parsed.activeProject;
			if (!isRecord(activeProject)) return emptyStoredState;
			const { id, title, path: projectPath, gitDir } = activeProject;
			if (
				typeof id !== "string" ||
				typeof title !== "string" ||
				typeof projectPath !== "string" ||
				typeof gitDir !== "string"
			)
				return emptyStoredState;
			return { activeProject: { id, title, path: projectPath, gitDir } };
		} catch {
			return emptyStoredState;
		}
	}

	async #writeStoredState(state: StoredState): Promise<void> {
		await fs.mkdir(path.dirname(this.statePath), { recursive: true });
		await fs.writeFile(this.statePath, `${JSON.stringify(state, null, 2)}\n`);
	}
}

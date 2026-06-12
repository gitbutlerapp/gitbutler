import { checkpointMessage, createDiffSpec } from "./checkpoints.js";
import {
	createInitialCheckpointCommit,
	discoverRepository,
	encodeProjectHandle,
	ensureGitRepository,
	listCheckpointCommits,
	projectTitleFromPath,
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
import type { Logger } from "./logger.js";
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
		private readonly logger: Logger,
	) {}

	async initialize(): Promise<HowStatus> {
		this.logger.info("Initializing How service", { statePath: this.statePath });
		const stored = await this.#readStoredState();
		if (stored.activeProject) {
			this.logger.info("Resuming active project", stored.activeProject);
			this.#status = {
				...this.#status,
				project: stored.activeProject,
				saveState: "watching",
				message: "Watching for changes",
			};
			try {
				await this.#refreshTimeline();
				await this.#startWatching(stored.activeProject);
			} catch (error) {
				this.logger.error("Failed to resume active project", error, stored.activeProject);
				this.#setError(error);
			}
		}
		this.#emit();
		return this.getStatus();
	}

	getStatus(): HowStatus {
		return this.#status;
	}

	async openProjectFromPath(selectedPath: string): Promise<HowStatus> {
		this.logger.info("Opening selected project", { selectedPath });
		const { gitDir, worktreePath } = await discoverRepository(selectedPath);
		this.logger.info("Resolved selected project", { selectedPath, gitDir, worktreePath });
		return await this.#activateProject({ gitDir, worktreePath });
	}

	async startProjectAtPath(selectedPath: string): Promise<HowStatus> {
		this.logger.info("Starting selected project", { selectedPath });
		const { gitDir, worktreePath } = await ensureGitRepository(selectedPath);
		this.logger.info("Resolved started project", { selectedPath, gitDir, worktreePath });
		return await this.#activateProject({ gitDir, worktreePath });
	}

	async createCheckpointNow(): Promise<HowStatus> {
		await this.#createCheckpoint();
		return this.getStatus();
	}

	async deleteProject(): Promise<HowStatus> {
		this.logger.info("Deleting active project from How", this.#status.project);
		await this.stop();
		this.#status = {
			project: null,
			saveState: "idle",
			message: null,
			lastSavedAt: null,
			checkpoints: [],
		};
		await this.#writeStoredState({ activeProject: null });
		this.#emit();
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

		this.logger.info("Activating project", project);
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
		this.logger.info("Starting watcher", project);
		this.#watcher = await watcherStart(project.id, (err: Error | null, event: WatcherEvent) => {
			if (err) {
				this.logger.error("Watcher callback error", err, project);
				this.#setError(err);
				return;
			}
			this.logger.info("Watcher event", { projectId: project.id, name: event.name });
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
		this.logger.info("Creating checkpoint", project);
		this.#status = {
			...this.#status,
			saveState: "saving",
			message: "Saving",
		};
		this.#emit();

		try {
			const worktreeChanges = await changesInWorktree(project.id);
			this.logger.info("Loaded worktree changes for checkpoint", {
				projectId: project.id,
				changeCount: worktreeChanges.changes.length,
				ignoredChangeCount: worktreeChanges.ignoredChanges.length,
				assignmentsError: worktreeChanges.assignmentsError,
				dependenciesError: worktreeChanges.dependenciesError,
			});
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
				this.logger.info("Creating checkpoint via but-sdk commitCreate", {
					projectId: project.id,
					message,
					changeCount: changes.length,
					placement,
				});
				const result = await commitCreate(
					project.id,
					placement.relativeTo,
					placement.side,
					changes,
					message,
					false,
				);
				this.logger.info("Checkpoint commitCreate result", {
					projectId: project.id,
					newCommit: result.newCommit,
					rejectedChangeCount: result.rejectedChanges.length,
				});
				if (result.rejectedChanges.length > 0 || result.newCommit === null)
					throw new Error("Some project changes could not be saved.");
			} else {
				this.logger.info("Creating initial checkpoint via git commit", {
					projectId: project.id,
					message,
					changeCount: changes.length,
				});
				const commitId = await createInitialCheckpointCommit(project.path, message);
				this.logger.info("Initial checkpoint result", { projectId: project.id, commitId });
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
			this.logger.error("Failed to create checkpoint", error, project);
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

		this.logger.info("Refreshing checkpoint timeline", project);
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
		this.logger.error("Setting user-visible error", error, {
			project: this.#status.project,
			message: plainErrorMessage(error),
		});
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

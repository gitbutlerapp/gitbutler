import { checkpointMessage } from "./checkpoints.js";
import {
	createCheckpointCommit,
	discoverRepository,
	encodeProjectHandle,
	ensureGitRepository,
	hasWorktreeChanges,
	listCheckpointCommits,
	projectTitleFromPath,
	resetToCommit,
} from "./git.js";
import {
	howIpcChannels,
	type BrowsingSession,
	type Checkpoint,
	type HowStatus,
	type ProjectSummary,
} from "./ipc.js";
import { plainErrorMessage } from "./plain-error.js";
import { watcherStart, type WatcherHandle, type WatcherEvent } from "@gitbutler/but-sdk";
import fs from "node:fs/promises";
import path from "node:path";
import type { Logger } from "./logger.js";
import type { BrowserWindow } from "electron";

const checkpointLimit = 50;
const defaultCheckpointQuietPeriodMs = 10_000;
const browsingDirtyPollMs = 500;

function checkpointQuietPeriodMs(): number {
	const override = process.env.HOW_CHECKPOINT_QUIET_MS;
	if (!override) return defaultCheckpointQuietPeriodMs;
	const parsed = Number(override);
	return Number.isFinite(parsed) && parsed >= 0 ? parsed : defaultCheckpointQuietPeriodMs;
}

type StoredState = {
	activeProject: ProjectSummary | null;
	browsing: BrowsingSession | null;
};

const emptyStoredState: StoredState = {
	activeProject: null,
	browsing: null,
};

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null;
}

async function sleep(milliseconds: number): Promise<void> {
	await new Promise((resolve) => setTimeout(resolve, milliseconds));
}

export class HowService {
	#status: HowStatus = {
		project: null,
		saveState: "idle",
		message: null,
		lastSavedAt: null,
		checkpoints: [],
		browsing: null,
	};
	#watcher: WatcherHandle | null = null;
	#debounce: NodeJS.Timeout | null = null;
	#browsingDirtyPoll: NodeJS.Timeout | null = null;
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
				message: stored.browsing ? "Browsing checkpoint" : "Watching for changes",
				checkpoints: stored.browsing?.checkpoints ?? [],
				browsing: stored.browsing,
			};
			try {
				if (stored.browsing) await this.#resumeBrowsingSession(stored.browsing);
				else await this.#refreshTimeline();
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

	async viewCheckpoint(
		checkpointId: string,
		options: { discardBrowsingChanges?: boolean } = {},
	): Promise<HowStatus> {
		const project = this.#status.project;
		if (!project) throw new Error("How could not find an open project.");

		this.logger.info("Viewing checkpoint", { project, checkpointId, options });
		this.#clearScheduledCheckpoint();
		await this.#waitForActiveSave();
		this.#clearScheduledCheckpoint();
		this.#saving = true;
		this.#dirtyWhileSaving = false;
		this.#status = {
			...this.#status,
			saveState: "saving",
			message: "Saving",
		};
		this.#emit();

		try {
			const currentBrowsing = this.#status.browsing;
			if (currentBrowsing) {
				if (
					currentBrowsing.dirty &&
					!options.discardBrowsingChanges &&
					currentBrowsing.currentCheckpointId !== checkpointId
				)
					throw new Error("Leave changes before viewing another checkpoint.");

				if (currentBrowsing.currentCheckpointId !== checkpointId)
					await resetToCommit(project.path, checkpointId);
				const browsing: BrowsingSession = {
					...currentBrowsing,
					currentCheckpointId: checkpointId,
					dirty: false,
				};
				this.#status = {
					...this.#status,
					saveState: "watching",
					message: "Browsing checkpoint",
					checkpoints: browsing.checkpoints,
					browsing,
				};
				this.#startBrowsingDirtyPolling(project);
				await this.#writeCurrentState();
				this.#emit();
				return this.getStatus();
			}

			await this.#saveCurrentWorkBeforeBrowsing(project);
			const originalCheckpoints = this.#status.checkpoints;
			const originalLatestCheckpointId = originalCheckpoints[0]?.id;
			if (!originalLatestCheckpointId) throw new Error("How could not find the latest checkpoint.");

			await resetToCommit(project.path, checkpointId);
			const browsing: BrowsingSession = {
				originalLatestCheckpointId,
				currentCheckpointId: checkpointId,
				checkpoints: originalCheckpoints,
				dirty: false,
				startedAt: Date.now(),
			};
			await this.#refreshTimeline();
			this.#status = {
				...this.#status,
				saveState: "watching",
				message: "Browsing checkpoint",
				checkpoints: browsing.checkpoints,
				browsing,
			};
			this.#startBrowsingDirtyPolling(project);
			await this.#writeCurrentState();
			this.#emit();
			return this.getStatus();
		} catch (error) {
			this.logger.error("Failed to view checkpoint", error, { project, checkpointId });
			this.#status = {
				...this.#status,
				saveState: "error",
				message: "How could not view that checkpoint.",
			};
			this.#emit();
			return this.getStatus();
		} finally {
			this.#saving = false;
		}
	}

	async continueFromCheckpoint(): Promise<HowStatus> {
		const project = this.#status.project;
		const browsing = this.#status.browsing;
		if (!project || !browsing) return this.getStatus();

		this.logger.info("Continuing from browsed checkpoint", { project, browsing });
		this.#clearScheduledCheckpoint();
		await this.#waitForActiveSave();
		this.#clearScheduledCheckpoint();
		this.#saving = true;
		this.#dirtyWhileSaving = false;
		this.#status = {
			...this.#status,
			saveState: "saving",
			message: "Saving",
		};
		this.#emit();

		try {
			if (browsing.dirty) {
				const message = checkpointMessage(new Date());
				this.logger.info("Creating checkpoint from browsing edits", {
					projectId: project.id,
					worktreePath: project.path,
					message,
					browsing,
				});
				await createCheckpointCommit(project.path, message);
			}
			this.#status = {
				...this.#status,
				browsing: null,
			};
			this.#stopBrowsingDirtyPolling();
			await this.#refreshTimeline();
			this.#status = {
				...this.#status,
				saveState: "saved",
				message: "Saved",
				lastSavedAt: Date.now(),
			};
			await this.#writeCurrentState();
			this.#emit();
			return this.getStatus();
		} catch (error) {
			this.logger.error("Failed to continue from checkpoint", error, { project, browsing });
			this.#status = {
				...this.#status,
				saveState: "error",
				message: "How could not continue from here.",
			};
			this.#emit();
			return this.getStatus();
		} finally {
			this.#saving = false;
		}
	}

	async returnToLatest(
		options: { discardBrowsingChanges?: boolean } = {},
	): Promise<HowStatus> {
		const project = this.#status.project;
		const browsing = this.#status.browsing;
		if (!project || !browsing) return this.getStatus();
		if (browsing.dirty && !options.discardBrowsingChanges)
			throw new Error("Leave changes before returning to latest.");

		this.logger.info("Returning to latest checkpoint", { project, browsing, options });
		this.#clearScheduledCheckpoint();
		await this.#waitForActiveSave();
		this.#clearScheduledCheckpoint();
		this.#saving = true;
		this.#dirtyWhileSaving = false;
		this.#status = {
			...this.#status,
			saveState: "saving",
			message: "Saving",
		};
		this.#emit();

		try {
			await resetToCommit(project.path, browsing.originalLatestCheckpointId);
			this.#status = {
				...this.#status,
				browsing: null,
			};
			this.#stopBrowsingDirtyPolling();
			await this.#refreshTimeline();
			this.#status = {
				...this.#status,
				saveState: "saved",
				message: "Returned to latest",
				lastSavedAt: Date.now(),
			};
			await this.#writeCurrentState();
			this.#emit();
			return this.getStatus();
		} catch (error) {
			this.logger.error("Failed to return to latest checkpoint", error, { project, browsing });
			this.#status = {
				...this.#status,
				saveState: "error",
				message: "How could not return to latest.",
			};
			this.#emit();
			return this.getStatus();
		} finally {
			this.#saving = false;
		}
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
			browsing: null,
		};
		await this.#writeStoredState({ activeProject: null, browsing: null });
		this.#emit();
		return this.getStatus();
	}

	async stop(): Promise<void> {
		this.#clearScheduledCheckpoint();
		this.#stopBrowsingDirtyPolling();
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
			browsing: null,
		};
		await this.#writeCurrentState();
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
			if (this.#eventShouldScheduleCheckpoint(event)) void this.#handleWorktreeChangeEvent(project);
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
		if (this.#status.browsing) return;
		this.#clearScheduledCheckpoint();
		this.#status = {
			...this.#status,
			saveState: "pending",
			message: "Saving soon",
		};
		this.#emit();
		this.#debounce = setTimeout(() => {
			this.#debounce = null;
			void this.#createCheckpoint();
		}, checkpointQuietPeriodMs());
	}

	async #handleWorktreeChangeEvent(project: ProjectSummary): Promise<void> {
		if (this.#saving) {
			if (!this.#status.browsing) this.#dirtyWhileSaving = true;
			return;
		}
		if (this.#status.browsing) {
			await this.#markBrowsingDirtyIfNeeded(project);
			return;
		}
		this.#scheduleCheckpoint();
	}

	#clearScheduledCheckpoint(): void {
		if (this.#debounce) clearTimeout(this.#debounce);
		this.#debounce = null;
	}

	async #waitForActiveSave(): Promise<void> {
		while (this.#saving) await sleep(25);
	}

	async #createCheckpoint(): Promise<void> {
		const project = this.#status.project;
		if (!project) return;
		if (this.#status.browsing) return;

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
			const message = checkpointMessage(new Date());
			this.logger.info("Creating checkpoint via git commit", {
				projectId: project.id,
				worktreePath: project.path,
				message,
			});
			const commitId = await createCheckpointCommit(project.path, message);
			this.logger.info("Git checkpoint result", { projectId: project.id, commitId });
			if (commitId === null) {
				this.#status = {
					...this.#status,
					saveState: "watching",
					message: "Watching for changes",
				};
				this.#emit();
				return;
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

	async #saveCurrentWorkBeforeBrowsing(project: ProjectSummary): Promise<void> {
		const message = checkpointMessage(new Date());
		this.logger.info("Creating checkpoint before browsing", {
			projectId: project.id,
			worktreePath: project.path,
			message,
		});
		const commitId = await createCheckpointCommit(project.path, message);
		this.logger.info("Checkpoint before browsing result", { projectId: project.id, commitId });
		await this.#refreshTimeline();
	}

	async #resumeBrowsingSession(browsing: BrowsingSession): Promise<void> {
		const project = this.#status.project;
		if (!project) return;
		const dirty = await hasWorktreeChanges(project.path);
		const resumedBrowsing = {
			...browsing,
			dirty,
		};
		this.#status = {
			...this.#status,
			saveState: "watching",
			message: dirty ? "Changes made while browsing" : "Browsing checkpoint",
			checkpoints: resumedBrowsing.checkpoints,
			browsing: resumedBrowsing,
		};
		this.#startBrowsingDirtyPolling(project);
		await this.#writeCurrentState();
	}

	async #markBrowsingDirtyIfNeeded(project: ProjectSummary): Promise<void> {
		const browsing = this.#status.browsing;
		if (!browsing || browsing.dirty) return;
		const dirty = await hasWorktreeChanges(project.path);
		if (!dirty) return;
		const nextBrowsing = {
			...browsing,
			dirty: true,
		};
		this.logger.info("Browsing checkpoint has local changes", {
			projectId: project.id,
			currentCheckpointId: browsing.currentCheckpointId,
		});
		this.#status = {
			...this.#status,
			saveState: "watching",
			message: "Changes made while browsing",
			browsing: nextBrowsing,
			checkpoints: nextBrowsing.checkpoints,
		};
		await this.#writeCurrentState();
		this.#emit();
	}

	#startBrowsingDirtyPolling(project: ProjectSummary): void {
		this.#stopBrowsingDirtyPolling();
		this.#browsingDirtyPoll = setInterval(() => {
			if (!this.#status.browsing || this.#saving) return;
			void this.#markBrowsingDirtyIfNeeded(project);
		}, browsingDirtyPollMs);
	}

	#stopBrowsingDirtyPolling(): void {
		if (this.#browsingDirtyPoll) clearInterval(this.#browsingDirtyPoll);
		this.#browsingDirtyPoll = null;
	}

	async #refreshTimeline(): Promise<void> {
		const project = this.#status.project;
		if (!project) return;

		this.logger.info("Refreshing checkpoint timeline", project);
		const commits = await listCheckpointCommits(project.path, checkpointLimit);
		this.logger.info("Loaded checkpoint timeline", {
			projectId: project.id,
			checkpointCount: commits.length,
		});
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
			const browsing = this.#parseBrowsingSession(parsed.browsing);
			return {
				activeProject: { id, title, path: projectPath, gitDir },
				browsing,
			};
		} catch {
			return emptyStoredState;
		}
	}

	#parseBrowsingSession(value: unknown): BrowsingSession | null {
		if (!isRecord(value)) return null;
		const {
			originalLatestCheckpointId,
			currentCheckpointId,
			checkpoints,
			dirty,
			startedAt,
		} = value;
		if (
			typeof originalLatestCheckpointId !== "string" ||
			typeof currentCheckpointId !== "string" ||
			!Array.isArray(checkpoints) ||
			typeof dirty !== "boolean" ||
			typeof startedAt !== "number"
		)
			return null;

		const parsedCheckpoints = checkpoints
			.map((checkpoint): Checkpoint | null => {
				if (!isRecord(checkpoint)) return null;
				const { id, title, createdAt } = checkpoint;
				if (
					typeof id !== "string" ||
					typeof title !== "string" ||
					typeof createdAt !== "number"
				)
					return null;
				return { id, title, createdAt };
			})
			.filter((checkpoint): checkpoint is Checkpoint => checkpoint !== null);

		if (parsedCheckpoints.length === 0) return null;
		return {
			originalLatestCheckpointId,
			currentCheckpointId,
			checkpoints: parsedCheckpoints,
			dirty,
			startedAt,
		};
	}

	async #writeCurrentState(): Promise<void> {
		await this.#writeStoredState({
			activeProject: this.#status.project,
			browsing: this.#status.browsing,
		});
	}

	async #writeStoredState(state: StoredState): Promise<void> {
		await fs.mkdir(path.dirname(this.statePath), { recursive: true });
		await fs.writeFile(this.statePath, `${JSON.stringify(state, null, 2)}\n`);
	}
}

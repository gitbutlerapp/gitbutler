import { checkpointMessageForStagedChanges } from "./checkpoint-summarizer.js";
import {
	createCheckpointCommit,
	discoverRepository,
	ensureGitRepository,
	hasWorktreeChanges,
	listCheckpointCommits,
	readProjectSettings,
	resetToCommit,
	writeProjectSettings,
	type GitRepository,
} from "./git.js";
import {
	howIpcChannels,
	type BrowsingSession,
	type Checkpoint,
	type HowStatus,
	type ProjectSettings,
	type ProjectSummary,
} from "./ipc.js";
import { plainErrorMessage } from "./plain-error.js";
import { defaultProjectSettings, normalizeProjectSettings } from "./settings.js";
import { watcherStart, type WatcherHandle, type WatcherEvent } from "@gitbutler/but-sdk";
import fs from "node:fs/promises";
import path from "node:path";
import type { Logger } from "./logger.js";
import type { BrowserWindow } from "electron";

const checkpointLimit = 50;
const fallbackCheckpointQuietPeriodMs = 10_000;
const browsingDirtyPollMs = 500;
const internalGitOperationQuietMs = 1_000;

function defaultCheckpointQuietPeriodMs(): number {
	const override = process.env.HOW_CHECKPOINT_QUIET_MS;
	if (!override) return fallbackCheckpointQuietPeriodMs;
	const parsed = Number(override);
	return Number.isFinite(parsed) && parsed >= 0 ? parsed : fallbackCheckpointQuietPeriodMs;
}

function defaultSettings(): ProjectSettings {
	return {
		...defaultProjectSettings,
		checkpointDebounceMs: defaultCheckpointQuietPeriodMs(),
	};
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
		settings: defaultSettings(),
	};
	#watcher: WatcherHandle | null = null;
	#debounce: NodeJS.Timeout | null = null;
	#browsingDirtyPoll: NodeJS.Timeout | null = null;
	#postInternalGitCheck: NodeJS.Timeout | null = null;
	#saving = false;
	#dirtyWhileSaving = false;
	#internalGitOperation = false;
	#ignoreWatcherUntil = 0;

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
				this.#status = {
					...this.#status,
					settings: await readProjectSettings(stored.activeProject.id, defaultSettings()),
				};
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
		const project = await discoverRepository(selectedPath);
		this.logger.info("Resolved selected project", { selectedPath, project });
		return await this.#activateProject(project);
	}

	async startProjectAtPath(selectedPath: string): Promise<HowStatus> {
		this.logger.info("Starting selected project", { selectedPath });
		const project = await ensureGitRepository(selectedPath);
		this.logger.info("Resolved started project", { selectedPath, project });
		return await this.#activateProject(project);
	}

	async createCheckpointNow(): Promise<HowStatus> {
		await this.#createCheckpoint();
		return this.getStatus();
	}

	async saveProjectSettings(settings: ProjectSettings): Promise<HowStatus> {
		const project = this.#status.project;
		if (!project) throw new Error("How could not find an open project.");

		const normalized = normalizeProjectSettings(settings);
		this.logger.info("Saving project settings", { projectId: project.id, settings: normalized });
		try {
			await writeProjectSettings(project.id, normalized);
			const hadPendingSave = this.#debounce !== null;
			if (hadPendingSave) this.#clearScheduledCheckpoint();
			this.#status = {
				...this.#status,
				settings: normalized,
				saveState: "saved",
				message: "Saved",
			};
			await this.#writeCurrentState();
			this.#emit();
			if (hadPendingSave) this.#scheduleCheckpoint();
			return this.getStatus();
		} catch (error) {
			this.logger.error("Failed to save project settings", error, { project, settings });
			this.#status = {
				...this.#status,
				saveState: "error",
				message: "How could not save settings.",
			};
			this.#emit();
			return this.getStatus();
		}
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
					await this.#runInternalGitOperation(
						project,
						async () =>
							await resetToCommit(project.id, checkpointId, {
								discardChanges: options.discardBrowsingChanges ?? false,
							}),
					);
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

			await this.#runInternalGitOperation(
				project,
				async () => await resetToCommit(project.id, checkpointId),
			);
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
				this.logger.info("Creating checkpoint from browsing edits", {
					projectId: project.id,
					worktreePath: project.path,
					browsing,
				});
				await this.#runInternalGitOperation(
					project,
					async () =>
						await createCheckpointCommit(
							project.id,
							async () => await this.#checkpointMessageForStagedChanges(project),
						),
				);
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

	async returnToLatest(options: { discardBrowsingChanges?: boolean } = {}): Promise<HowStatus> {
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
			await this.#runInternalGitOperation(
				project,
				async () =>
					await resetToCommit(project.id, browsing.originalLatestCheckpointId, {
						discardChanges: options.discardBrowsingChanges ?? false,
					}),
			);
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
			settings: defaultSettings(),
		};
		await this.#writeStoredState({ activeProject: null, browsing: null });
		this.#emit();
		return this.getStatus();
	}

	async stop(): Promise<void> {
		this.#clearScheduledCheckpoint();
		this.#clearPostInternalGitCheck();
		this.#stopBrowsingDirtyPolling();
		if (this.#watcher) this.#watcher.stop();
		this.#watcher = null;
	}

	async #activateProject({
		id,
		title,
		gitDir,
		worktreePath,
	}: GitRepository): Promise<HowStatus> {
		const project: ProjectSummary = {
			id,
			title,
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
			settings: await readProjectSettings(project.id, defaultSettings()),
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
		}, this.#status.settings.checkpointDebounceMs);
	}

	async #handleWorktreeChangeEvent(project: ProjectSummary): Promise<void> {
		if (Date.now() < this.#ignoreWatcherUntil) return;
		if (this.#saving) {
			if (!this.#status.browsing && !this.#internalGitOperation) this.#dirtyWhileSaving = true;
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

		if (!(await hasWorktreeChanges(project.id))) {
			this.#status = {
				...this.#status,
				saveState: "watching",
				message: "Watching for changes",
			};
			this.#emit();
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
			this.logger.info("Creating checkpoint via git commit", {
				projectId: project.id,
				worktreePath: project.path,
			});
			const commitId = await this.#runInternalGitOperation(
				project,
				async () =>
					await createCheckpointCommit(
						project.id,
						async () => await this.#checkpointMessageForStagedChanges(project),
					),
			);
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
				if (await hasWorktreeChanges(project.id)) this.#scheduleCheckpoint();
			}
		}
	}

	async #saveCurrentWorkBeforeBrowsing(project: ProjectSummary): Promise<void> {
		if (!(await hasWorktreeChanges(project.id))) {
			this.logger.info("Skipping checkpoint before browsing because there are no changes", {
				projectId: project.id,
				worktreePath: project.path,
			});
			return;
		}

		this.logger.info("Creating checkpoint before browsing", {
			projectId: project.id,
			worktreePath: project.path,
		});
		const commitId = await this.#runInternalGitOperation(
			project,
			async () =>
				await createCheckpointCommit(
					project.id,
					async () => await this.#checkpointMessageForStagedChanges(project),
				),
		);
		this.logger.info("Checkpoint before browsing result", { projectId: project.id, commitId });
		await this.#refreshTimeline();
	}

	async #checkpointMessageForStagedChanges(project: ProjectSummary): Promise<string> {
		return await checkpointMessageForStagedChanges({
			agent: this.#status.settings.codingAgent,
			date: new Date(),
			logger: this.logger,
			projectId: project.id,
			worktreePath: project.path,
		});
	}

	async #runInternalGitOperation<T>(
		project: ProjectSummary,
		operation: () => Promise<T>,
	): Promise<T> {
		this.#internalGitOperation = true;
		try {
			return await operation();
		} finally {
			this.#internalGitOperation = false;
			this.#ignoreWatcherUntil = Date.now() + internalGitOperationQuietMs;
			this.#schedulePostInternalGitCheck(project);
		}
	}

	#schedulePostInternalGitCheck(project: ProjectSummary): void {
		this.#clearPostInternalGitCheck();
		this.#postInternalGitCheck = setTimeout(() => {
			this.#postInternalGitCheck = null;
			void this.#checkForChangesAfterInternalGitOperation(project);
		}, internalGitOperationQuietMs);
	}

	async #checkForChangesAfterInternalGitOperation(project: ProjectSummary): Promise<void> {
		if (this.#saving || this.#status.project?.id !== project.id) return;
		if (this.#status.browsing) {
			await this.#markBrowsingDirtyIfNeeded(project);
			return;
		}
		if (await hasWorktreeChanges(project.id)) this.#scheduleCheckpoint();
	}

	#clearPostInternalGitCheck(): void {
		if (this.#postInternalGitCheck) clearTimeout(this.#postInternalGitCheck);
		this.#postInternalGitCheck = null;
	}

	async #resumeBrowsingSession(browsing: BrowsingSession): Promise<void> {
		const project = this.#status.project;
		if (!project) return;
		const dirty = await hasWorktreeChanges(project.id);
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
		const dirty = await hasWorktreeChanges(project.id);
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
		const commits = await listCheckpointCommits(project.id, checkpointLimit);
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
		const { originalLatestCheckpointId, currentCheckpointId, checkpoints, dirty, startedAt } =
			value;
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
				if (typeof id !== "string" || typeof title !== "string" || typeof createdAt !== "number")
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

import { checkpointMessage } from "./checkpoints.js";
import {
	createCheckpointCommit,
	discoverRepository,
	encodeProjectHandle,
	ensureGitRepository,
	listCheckpointCommits,
	projectTitleFromPath,
} from "./git.js";
import { howIpcChannels, type Checkpoint, type HowStatus, type ProjectSummary } from "./ipc.js";
import { plainErrorMessage } from "./plain-error.js";
import { watcherStart, type WatcherHandle, type WatcherEvent } from "@gitbutler/but-sdk";
import fs from "node:fs/promises";
import path from "node:path";
import type { Logger } from "./logger.js";
import type { BrowserWindow } from "electron";

const checkpointLimit = 50;
const defaultCheckpointQuietPeriodMs = 10_000;

function checkpointQuietPeriodMs(): number {
	const override = process.env.HOW_CHECKPOINT_QUIET_MS;
	if (!override) return defaultCheckpointQuietPeriodMs;
	const parsed = Number(override);
	return Number.isFinite(parsed) && parsed >= 0 ? parsed : defaultCheckpointQuietPeriodMs;
}

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
		}, checkpointQuietPeriodMs());
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

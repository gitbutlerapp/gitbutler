import { capDiffForSummary, checkpointMessageForSavedCheckpoint } from "./checkpoint-summarizer.js";
import { checkpointMessage } from "./checkpoints.js";
import {
	checkpointDiffForCommit,
	createCheckpointCommit,
	createProjectBookmark,
	createProjectBookmarkFromCommit,
	deleteProjectBookmark,
	DirectPublishError,
	filterUnpublishedCommits,
	discoverRepository,
	ensureGitRepository,
	gitErrorDetails,
	hasAnyRemote,
	hasGithubDestination,
	hasWorktreeChanges,
	listProjectBookmarks,
	listCheckpointCommits,
	publishDirect,
	readPublishMode,
	readProjectSettings,
	renameProjectBookmark,
	refreshSharedProject,
	resetToCommit,
	sanitizedRepositoryName,
	switchProjectBookmark,
	updateCheckpointMessageByChangeId,
	updateProjectBookmark,
	writePublishMode,
	writeProjectSettings,
	type CreatedCheckpoint,
	type GitRepository,
} from "./git.js";
import {
	howIpcChannels,
	type Bookmark,
	type BrowsingSession,
	type Checkpoint,
	type HowStatus,
	type PublishProjectInput,
	type PublishProjectResult,
	type ProjectSettings,
	type ProjectSummary,
	type SharedProjectStatus,
} from "./ipc.js";
import { plainErrorMessage } from "./plain-error.js";
import { defaultProjectSettings, normalizeProjectSettings } from "./settings.js";
import { watcherStart, type WatcherHandle, type WatcherEvent } from "@gitbutler/but-sdk";
import fs from "node:fs/promises";
import path from "node:path";
import type { GithubService } from "./github-service.js";
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

function defaultSharedProjectStatus(): SharedProjectStatus {
	return {
		state: "unknown",
		lastCheckedAt: null,
		message: null,
	};
}

function effectiveFetchIntervalMs(settings: ProjectSettings): number {
	const override = process.env.HOW_SHARED_FETCH_INTERVAL_MS;
	if (override) {
		const parsed = Number(override);
		if (Number.isFinite(parsed) && parsed >= 0) return parsed;
	}
	return settings.fetchIntervalMs;
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
		bookmarks: [],
		browsing: null,
		settings: defaultSettings(),
		sharedProject: defaultSharedProjectStatus(),
	};
	#watcher: WatcherHandle | null = null;
	#debounce: NodeJS.Timeout | null = null;
	#browsingDirtyPoll: NodeJS.Timeout | null = null;
	#postInternalGitCheck: NodeJS.Timeout | null = null;
	#sharedProjectFetch: NodeJS.Timeout | null = null;
	#saving = false;
	#dirtyWhileSaving = false;
	#internalGitOperation = false;
	#ignoreWatcherUntil = 0;
	#gitOperationQueue: Promise<void> = Promise.resolve();
	#summaryGeneration = 0;
	#summaryAbortControllers = new Set<AbortController>();

	constructor(
		private readonly statePath: string,
		private readonly getWindows: () => Array<BrowserWindow>,
		private readonly logger: Logger,
		private readonly github: GithubService,
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
					bookmarks: [],
					browsing: stored.browsing,
				};
			try {
				this.#status = {
					...this.#status,
					settings: await readProjectSettings(
						stored.activeProject.id,
						stored.activeProject.path,
						defaultSettings(),
					),
				};
				if (stored.browsing) {
					await this.#resumeBrowsingSession(stored.browsing);
					await this.#refreshBookmarks();
				} else {
					await this.#refreshSharedProjectStatus(stored.activeProject, { fetch: true });
					await this.#refreshProjectLists();
				}
				await this.#startWatching(stored.activeProject);
				this.#startSharedProjectFetching(stored.activeProject);
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

	async publishProject(input: PublishProjectInput = {}): Promise<PublishProjectResult> {
		const project = this.#status.project;
		if (!project) throw new Error("How could not find an open project.");
		if (this.#status.browsing)
			throw new Error("Continue from here or return to latest before publishing.");

		if (!(await this.github.hasCredential()))
			return { type: "needsGithubLogin", status: this.getStatus() };

		const hasRemote = await hasAnyRemote(project.path);
		const hasGithubRemote = await hasGithubDestination(project.path);
		if (hasRemote && !hasGithubRemote)
			return this.#publishFailure(
				"This project already publishes somewhere How does not support yet.",
			);

		let destinationUrl = input.githubRepositoryCloneUrl;
		if (!hasRemote && !destinationUrl && input.createGithubRepositoryName) {
			const repository = await this.github.createRepository(input.createGithubRepositoryName);
			destinationUrl = repository.cloneUrl;
		}

		if (!hasRemote && !destinationUrl) {
			return {
				type: "needsGithubRepository",
				status: this.getStatus(),
				defaultRepositoryName: sanitizedRepositoryName(project.title),
				repositories: null,
			};
		}

		const configuredPublishMode = await readPublishMode(project.path);
		if (configuredPublishMode !== "direct")
			await this.#runInternalGitOperation(
				project,
				async () => await writePublishMode(project.path, "direct"),
			);

		this.logger.info("Publishing project to GitHub", {
			projectId: project.id,
			worktreePath: project.path,
			hasDestinationUrl: Boolean(destinationUrl),
		});
		this.#clearScheduledCheckpoint();
		await this.#waitForActiveSave();
		this.#clearScheduledCheckpoint();
		this.#saving = true;
		this.#dirtyWhileSaving = false;
		this.#status = {
			...this.#status,
			saveState: "saving",
			message: "Publishing",
		};
		this.#emit();

		try {
			await this.#createCheckpointForPublish(project);
			const githubToken = await this.github.tokenForGit();
			const result = await this.#runInternalGitOperation(
				project,
				async () => await publishDirect(project.path, { destinationUrl, githubToken }),
			);
			this.logger.info("GitHub publish result", { projectId: project.id, result });
			if (result.type === "needsDestination") {
				this.#status = {
					...this.#status,
					saveState: "watching",
					message: "Choose a GitHub project",
				};
				this.#emit();
				return {
					type: "needsGithubRepository",
					status: this.getStatus(),
					defaultRepositoryName: sanitizedRepositoryName(project.title),
					repositories: null,
				};
			}

			this.#cancelPendingSummaryUpdates("publish");
			await this.#refreshSharedProjectStatus(project, { fetch: true });
			await this.#refreshProjectLists();
			this.#status = {
				...this.#status,
				saveState: "saved",
				message: "Published just now",
				lastSavedAt: Date.now(),
			};
			this.#emit();
			return { type: "published", status: this.getStatus() };
		} catch (error) {
			this.logger.error("Failed to publish project to GitHub", error, {
				project,
				details: gitErrorDetails(error),
			});
			this.#status = {
				...this.#status,
				saveState: "error",
				message:
					error instanceof DirectPublishError
						? error.message
						: error instanceof Error && error.message === "How could not save before publishing."
							? error.message
							: "How could not publish to the shared project.",
			};
			this.#emit();
			return { type: "failed", status: this.getStatus() };
		} finally {
			this.#saving = false;
			if (this.#dirtyWhileSaving) {
				this.#dirtyWhileSaving = false;
				if (await hasWorktreeChanges(project.id)) this.#scheduleCheckpoint();
			}
		}
	}

	async loginToGithub() {
		return await this.github.login();
	}

	async listGithubRepositories() {
		return await this.github.listRepositories();
	}

	#publishFailure(message: string): PublishProjectResult {
		this.#status = {
			...this.#status,
			saveState: "error",
			message,
		};
		this.#emit();
		return { type: "failed", status: this.getStatus() };
	}

	async saveProjectSettings(settings: ProjectSettings): Promise<HowStatus> {
		const project = this.#status.project;
		if (!project) throw new Error("How could not find an open project.");

		const normalized = normalizeProjectSettings(settings);
		this.logger.info("Saving project settings", { projectId: project.id, settings: normalized });
		try {
			await writeProjectSettings(project.id, project.path, normalized);
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
			this.#startSharedProjectFetching(project);
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

	async createBookmark(name: string): Promise<HowStatus> {
		const project = this.#status.project;
		if (!project) throw new Error("How could not find an open project.");

		this.logger.info("Creating bookmark", { project, name });
		this.#clearScheduledCheckpoint();
		await this.#waitForActiveSave();
		this.#clearScheduledCheckpoint();
		this.#saving = true;
		this.#dirtyWhileSaving = false;
		this.#status = { ...this.#status, saveState: "saving", message: "Saving" };
		this.#emit();

		try {
			await this.#saveCurrentWorkBeforeBookmark(project, "normal");
			await this.#runInternalGitOperation(
				project,
				async () => await createProjectBookmark(project.id, name, "user"),
			);
			await this.#refreshProjectLists();
			this.#status = {
				...this.#status,
				saveState: "saved",
				message: "Bookmarked",
				lastSavedAt: Date.now(),
			};
			await this.#writeCurrentState();
			this.#emit();
			return this.getStatus();
		} catch (error) {
			this.logger.error("Failed to create bookmark", error, { project, name });
			this.#status = {
				...this.#status,
				saveState: "error",
				message: "How could not create that bookmark.",
			};
			this.#emit();
			return this.getStatus();
		} finally {
			this.#saving = false;
		}
	}

	async createBookmarkFromCheckpoint(name: string, checkpointId: string): Promise<HowStatus> {
		const project = this.#status.project;
		const browsing = this.#status.browsing;
		if (!project) throw new Error("How could not find an open project.");
		if (!browsing || browsing.dirty || browsing.currentCheckpointId !== checkpointId)
			throw new Error("How could not bookmark that checkpoint.");

		this.logger.info("Creating bookmark from checkpoint", { project, name, checkpointId });
		try {
			await this.#runInternalGitOperation(
				project,
				async () => await createProjectBookmarkFromCommit(project.id, name, "user", checkpointId),
			);
			await this.#refreshBookmarks();
			this.#status = {
				...this.#status,
				saveState: "saved",
				message: "Bookmarked",
				lastSavedAt: Date.now(),
			};
			await this.#writeCurrentState();
			this.#emit();
			return this.getStatus();
		} catch (error) {
			this.logger.error("Failed to create bookmark from checkpoint", error, {
				project,
				name,
				checkpointId,
			});
			this.#status = {
				...this.#status,
				saveState: "error",
				message: "How could not create that bookmark.",
			};
			this.#emit();
			return this.getStatus();
		}
	}

	async switchBookmark(bookmarkId: string): Promise<HowStatus> {
		const project = this.#status.project;
		if (!project) throw new Error("How could not find an open project.");
		const bookmark = this.#status.bookmarks.find((candidate) => candidate.id === bookmarkId);
		if (!bookmark) throw new Error("How could not find that bookmark.");
		if (bookmark.isCurrent) return this.getStatus();

		this.logger.info("Switching bookmark", { project, bookmarkId });
		this.#cancelPendingSummaryUpdates("switch bookmark");
		this.#clearScheduledCheckpoint();
		await this.#waitForActiveSave();
		this.#clearScheduledCheckpoint();
		this.#saving = true;
		this.#dirtyWhileSaving = false;

		try {
			await this.#saveCurrentWorkBeforeBookmark(project, "fast");
			await this.#refreshBookmarks();
			await this.#preserveCurrentStateAsBookmarkIfMissing(project, bookmark.name);
			await this.#runInternalGitOperation(
				project,
				async () => await switchProjectBookmark(project.id, bookmarkId),
			);
			this.#status = { ...this.#status, browsing: null };
			this.#stopBrowsingDirtyPolling();
			await this.#refreshSharedProjectStatus(project, { fetch: false });
			await this.#refreshProjectLists();
			await this.#writeCurrentState();
			this.#emit();
			return this.getStatus();
		} catch (error) {
			this.logger.error("Failed to switch bookmark", error, { project, bookmarkId });
			this.#status = {
				...this.#status,
				saveState: "error",
				message: "How could not switch bookmarks.",
			};
			this.#emit();
			return this.getStatus();
		} finally {
			this.#saving = false;
		}
	}

	async updateBookmark(bookmarkId: string): Promise<HowStatus> {
		const project = this.#status.project;
		if (!project) throw new Error("How could not find an open project.");
		if (this.#status.browsing) throw new Error("Continue from here before updating a bookmark.");

		this.logger.info("Updating bookmark", { project, bookmarkId });
		this.#clearScheduledCheckpoint();
		await this.#waitForActiveSave();
		this.#clearScheduledCheckpoint();
		this.#saving = true;
		this.#dirtyWhileSaving = false;
		this.#status = { ...this.#status, saveState: "saving", message: "Saving" };
		this.#emit();

		try {
			await this.#saveCurrentWorkBeforeBookmark(project, "fast");
			await this.#runInternalGitOperation(
				project,
				async () => await updateProjectBookmark(project.id, bookmarkId),
			);
			await this.#refreshProjectLists();
			this.#status = {
				...this.#status,
				saveState: "saved",
				message: "Bookmark updated",
				lastSavedAt: Date.now(),
			};
			await this.#writeCurrentState();
			this.#emit();
			return this.getStatus();
		} catch (error) {
			this.logger.error("Failed to update bookmark", error, { project, bookmarkId });
			this.#status = {
				...this.#status,
				saveState: "error",
				message: "How could not update that bookmark.",
			};
			this.#emit();
			return this.getStatus();
		} finally {
			this.#saving = false;
		}
	}

	async renameBookmark(bookmarkId: string, name: string): Promise<HowStatus> {
		const project = this.#status.project;
		if (!project) throw new Error("How could not find an open project.");
		try {
			await renameProjectBookmark(project.id, bookmarkId, name);
			await this.#refreshBookmarks();
			this.#emit();
			return this.getStatus();
		} catch (error) {
			this.logger.error("Failed to rename bookmark", error, { project, bookmarkId, name });
			this.#status = {
				...this.#status,
				saveState: "error",
				message: "How could not rename that bookmark.",
			};
			this.#emit();
			return this.getStatus();
		}
	}

	async deleteBookmark(bookmarkId: string): Promise<HowStatus> {
		const project = this.#status.project;
		if (!project) throw new Error("How could not find an open project.");
		try {
			await deleteProjectBookmark(project.id, bookmarkId);
			await this.#refreshBookmarks();
			this.#emit();
			return this.getStatus();
		} catch (error) {
			this.logger.error("Failed to delete bookmark", error, { project, bookmarkId });
			this.#status = {
				...this.#status,
				saveState: "error",
				message: "How could not delete that bookmark.",
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
		this.#cancelPendingSummaryUpdates("view checkpoint");
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
				await this.#refreshBookmarks();
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
			await this.#refreshBookmarks();
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
		this.#cancelPendingSummaryUpdates("continue from checkpoint");
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
			let continuedCheckpoint: CreatedCheckpoint | null = null;
			if (browsing.dirty) {
				this.logger.info("Creating checkpoint from browsing edits", {
					projectId: project.id,
					worktreePath: project.path,
					browsing,
				});
				continuedCheckpoint = await this.#runInternalGitOperation(
					project,
					async () => await createCheckpointCommit(project.id, checkpointMessage(new Date())),
				);
			}
			this.#status = {
				...this.#status,
				browsing: null,
			};
			this.#stopBrowsingDirtyPolling();
			await this.#refreshSharedProjectStatus(project, { fetch: false });
			await this.#refreshProjectLists();
			if (continuedCheckpoint) this.#enqueueCheckpointSummary(project, continuedCheckpoint);
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
		this.#cancelPendingSummaryUpdates("return to latest");
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
			await this.#refreshSharedProjectStatus(project, { fetch: false });
			await this.#refreshProjectLists();
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
		this.#cancelPendingSummaryUpdates("delete project");
		await this.stop();
		this.#status = {
			project: null,
			saveState: "idle",
			message: null,
			lastSavedAt: null,
			checkpoints: [],
			bookmarks: [],
			browsing: null,
			settings: defaultSettings(),
			sharedProject: defaultSharedProjectStatus(),
		};
		await this.#writeStoredState({ activeProject: null, browsing: null });
		this.#emit();
		return this.getStatus();
	}

	async stop(): Promise<void> {
		this.#cancelPendingSummaryUpdates("stop");
		this.#clearScheduledCheckpoint();
		this.#clearPostInternalGitCheck();
		this.#stopSharedProjectFetching();
		this.#stopBrowsingDirtyPolling();
		if (this.#watcher) this.#watcher.stop();
		this.#watcher = null;
	}

	async #activateProject({ id, title, gitDir, worktreePath }: GitRepository): Promise<HowStatus> {
		const project: ProjectSummary = {
			id,
			title,
			path: worktreePath,
			gitDir,
		};

		this.logger.info("Activating project", project);
		this.#cancelPendingSummaryUpdates("activate project");
		await this.stop();
		this.#status = {
			project,
			saveState: "watching",
			message: "Watching for changes",
			lastSavedAt: null,
			checkpoints: [],
			bookmarks: [],
			browsing: null,
			settings: await readProjectSettings(project.id, project.path, defaultSettings()),
			sharedProject: defaultSharedProjectStatus(),
		};
		await this.#writeCurrentState();
		await this.#refreshSharedProjectStatus(project, { fetch: true });
		await this.#refreshProjectLists();
		await this.#startWatching(project);
		this.#startSharedProjectFetching(project);
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
				async () => await createCheckpointCommit(project.id, checkpointMessage(new Date())),
			);
			this.logger.info("Git checkpoint result", { projectId: project.id, checkpoint: commitId });
			if (commitId === null) {
				this.#status = {
					...this.#status,
					saveState: "watching",
					message: "Watching for changes",
				};
				this.#emit();
				return;
			}

			await this.#refreshProjectLists();
			this.#enqueueCheckpointSummary(project, commitId);
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
			async () => await createCheckpointCommit(project.id, checkpointMessage(new Date())),
		);
		this.logger.info("Checkpoint before browsing result", { projectId: project.id, commitId });
		await this.#refreshProjectLists();
	}

	async #saveCurrentWorkBeforeBookmark(
		project: ProjectSummary,
		mode: "normal" | "fast",
	): Promise<void> {
		if (!(await hasWorktreeChanges(project.id))) return;

		this.logger.info("Creating checkpoint before bookmark operation", {
			projectId: project.id,
			worktreePath: project.path,
			mode,
		});
		const message = checkpointMessage(new Date());
		const commitId = await this.#runInternalGitOperation(
			project,
			async () => await createCheckpointCommit(project.id, message),
		);
		this.logger.info("Checkpoint before bookmark operation result", {
			projectId: project.id,
			commitId,
		});
		await this.#refreshProjectLists();
	}

	async #preserveCurrentStateAsBookmarkIfMissing(
		project: ProjectSummary,
		switchingToName: string,
	): Promise<void> {
		if (this.#status.bookmarks.some((bookmark) => bookmark.isCurrent)) return;
		const fallback = checkpointMessage(new Date()).replace(/^Checkpoint:\s*/, "");
		const name = switchingToName.trim()
			? `Before switching to ${switchingToName.trim()}`
			: `Before switching: ${fallback}`;
		this.logger.info("Creating backup bookmark before switch", {
			projectId: project.id,
			worktreePath: project.path,
			name,
		});
		await this.#runInternalGitOperation(
			project,
			async () => await createProjectBookmark(project.id, name, "auto"),
		);
		await this.#refreshBookmarks();
	}

	async #createCheckpointForPublish(project: ProjectSummary): Promise<void> {
		if (!(await hasWorktreeChanges(project.id))) return;

		this.logger.info("Creating checkpoint before publishing", {
			projectId: project.id,
			worktreePath: project.path,
		});
		try {
			const commitId = await this.#runInternalGitOperation(
				project,
				async () => await createCheckpointCommit(project.id, checkpointMessage(new Date())),
			);
			this.logger.info("Checkpoint before publishing result", { projectId: project.id, commitId });
			if (commitId === null) return;
			await this.#refreshProjectLists();
		} catch (error) {
			this.logger.error("Failed to create checkpoint before publishing", error, { project });
			throw new Error("How could not save before publishing.");
		}
	}

	#cancelPendingSummaryUpdates(reason: string): void {
		this.#summaryGeneration += 1;
		for (const controller of this.#summaryAbortControllers) controller.abort();
		this.#summaryAbortControllers.clear();
		this.logger.info("Cancelled pending checkpoint summary updates", {
			reason,
			generation: this.#summaryGeneration,
		});
	}

	#enqueueCheckpointSummary(project: ProjectSummary, checkpoint: CreatedCheckpoint): void {
		const agent = this.#status.settings.codingAgent;
		if (agent === "none") return;
		const generation = this.#summaryGeneration;
		const controller = new AbortController();
		this.#summaryAbortControllers.add(controller);
		this.logger.info("Enqueued async checkpoint summary", {
			projectId: project.id,
			checkpoint,
			generation,
			agent,
		});
		void this.#runCheckpointSummaryJob(project, checkpoint, generation, controller);
	}

	async #runCheckpointSummaryJob(
		project: ProjectSummary,
		checkpoint: CreatedCheckpoint,
		generation: number,
		controller: AbortController,
	): Promise<void> {
		try {
			const rawDiff = await checkpointDiffForCommit(project.path, checkpoint.commitId);
			if (controller.signal.aborted || generation !== this.#summaryGeneration) return;
			const diff = capDiffForSummary(rawDiff.diff);
			const agent = this.#status.settings.codingAgent;
			if (agent === "none") return;
			const message = await checkpointMessageForSavedCheckpoint({
				agent,
				checkpoint,
				date: new Date(),
				diff: diff.diff,
				diffTruncated: diff.truncated,
				logger: this.logger,
				originalByteCount: rawDiff.originalByteCount,
				projectId: project.id,
				signal: controller.signal,
				worktreePath: project.path,
			});
			if (!message || controller.signal.aborted || generation !== this.#summaryGeneration) return;
			if (this.#status.project?.id !== project.id || this.#status.browsing) return;

			const result = await this.#runInternalGitOperation(
				project,
				async () =>
					await updateCheckpointMessageByChangeId(project.id, checkpoint.changeId, message),
			);
			this.logger.info("Async checkpoint summary update result", {
				projectId: project.id,
				checkpoint,
				result,
			});
			if (
				result.type === "updated" &&
				generation === this.#summaryGeneration &&
				this.#status.project?.id === project.id &&
				!this.#status.browsing
			) {
				await this.#refreshProjectLists();
				this.#emit();
			}
		} catch (error) {
			this.logger.error("Async checkpoint summary update failed", error, {
				projectId: project.id,
				checkpoint,
				generation,
			});
		} finally {
			this.#summaryAbortControllers.delete(controller);
		}
	}

	async #runInternalGitOperation<T>(
		project: ProjectSummary,
		operation: () => Promise<T>,
	): Promise<T> {
		const previous = this.#gitOperationQueue;
		let release!: () => void;
		this.#gitOperationQueue = new Promise<void>((resolve) => {
			release = resolve;
		});
		await previous;
		this.#internalGitOperation = true;
		try {
			return await operation();
		} finally {
			this.#internalGitOperation = false;
			this.#ignoreWatcherUntil = Date.now() + internalGitOperationQuietMs;
			this.#schedulePostInternalGitCheck(project);
			release();
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

	#startSharedProjectFetching(project: ProjectSummary): void {
		this.#stopSharedProjectFetching();
		const intervalMs = effectiveFetchIntervalMs(this.#status.settings);
		if (intervalMs <= 0) {
			this.logger.info("Shared project fetching is disabled", { projectId: project.id });
			return;
		}
		this.logger.info("Starting shared project fetch timer", {
			projectId: project.id,
			intervalMs,
		});
		this.#sharedProjectFetch = setInterval(() => {
			void this.#refreshSharedProjectFromTimer(project);
		}, intervalMs);
	}

	#stopSharedProjectFetching(): void {
		if (this.#sharedProjectFetch) clearInterval(this.#sharedProjectFetch);
		this.#sharedProjectFetch = null;
	}

	async #refreshSharedProjectFromTimer(project: ProjectSummary): Promise<void> {
		if (this.#status.project?.id !== project.id) return;
		if (this.#saving || this.#internalGitOperation) return;
		await this.#refreshSharedProjectStatus(project, { fetch: true });
		if (!this.#status.browsing) await this.#refreshTimeline();
		this.#emit();
	}

	async #refreshSharedProjectStatus(
		project: ProjectSummary,
		options: { fetch: boolean },
	): Promise<void> {
		try {
			this.logger.info("Refreshing shared project status", {
				projectId: project.id,
				fetch: options.fetch,
			});
			const sharedProject = await refreshSharedProject(project.path, { fetch: options.fetch });
			this.logger.info("Shared project status refreshed", { projectId: project.id, sharedProject });
			this.#status = {
				...this.#status,
				sharedProject,
			};
		} catch (error) {
			this.logger.error("Failed to refresh shared project status", error, {
				projectId: project.id,
				details: gitErrorDetails(error),
			});
			this.#status = {
				...this.#status,
				sharedProject: {
					state: "couldNotCheck",
					lastCheckedAt: this.#status.sharedProject.lastCheckedAt,
					message: "Could not check for updates",
				},
			};
		}
	}

	async #refreshTimeline(): Promise<void> {
		const project = this.#status.project;
		if (!project) return;

		this.logger.info("Refreshing checkpoint timeline", project);
		const commits = await filterUnpublishedCommits(
			project.path,
			await listCheckpointCommits(project.id, checkpointLimit),
		);
		this.logger.info("Loaded checkpoint timeline", {
			projectId: project.id,
			checkpointCount: commits.length,
			sharedProject: this.#status.sharedProject.state,
		});
		const checkpoints: Array<Checkpoint> = commits.map((commit) => ({
			id: commit.id,
			changeId: commit.changeId,
			title: commit.title,
			createdAt: commit.createdAt,
		}));
		this.#status = {
			...this.#status,
			checkpoints,
		};
	}

	async #refreshBookmarks(): Promise<void> {
		const project = this.#status.project;
		if (!project) return;

		this.logger.info("Refreshing bookmarks", project);
		const bookmarks: Array<Bookmark> = (await listProjectBookmarks(project.id)).map((bookmark) => ({
			id: bookmark.id,
			name: bookmark.name,
			targetCommitId: bookmark.targetCommitId,
			createdAt: bookmark.createdAt,
			updatedAt: bookmark.updatedAt,
			kind: bookmark.kind,
			isCurrent: bookmark.isCurrent,
		}));
		this.#status = {
			...this.#status,
			bookmarks,
		};
	}

	async #refreshProjectLists(): Promise<void> {
		await this.#refreshTimeline();
		await this.#refreshBookmarks();
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
				const { id, changeId, title, createdAt } = checkpoint;
				if (typeof id !== "string" || typeof title !== "string" || typeof createdAt !== "number")
					return null;
				return { id, changeId: typeof changeId === "string" ? changeId : null, title, createdAt };
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

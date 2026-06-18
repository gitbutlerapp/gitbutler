import { contextBridge, ipcRenderer } from "electron";
import type { LiteElectronApi, UpdateBranchNameResult, WatcherSubscribeResult } from "./ipc";
import type { UpdateDownloadedEvent } from "electron-updater";
import type {
	CommitAbsorption,
	ApplyOutcome,
	BranchCheckoutResult,
	BranchCreateResult,
	BranchDetails,
	BranchListing,
	CommitDetails,
	DiffSpec,
	Editor,
	ForgeReview,
	ProjectForFrontend,
	PushResult,
	RefInfo,
	TreeChanges,
	CommitCreateResult,
	CommitDiscardResult,
	CommitInsertBlankResult,
	CommitMoveResult,
	CommitRewordResult,
	CommitSquashResult,
	MoveBranchResult,
	MoveChangesResult,
	UnifiedPatch,
	WatcherEvent,
	WorktreeChanges,
	WorkspaceState,
	UncommitResult,
	Snapshot,
	AskpassPromptEvent,
} from "@gitbutler/but-sdk";

/**
 * The map of subscription IDs to channels and callbacks.
 *
 * This is needed in order to maintain separate changes for each subscription.
 * The subscription ID is known to the UI, but the channel is not.
 */
const watcherListenerBySubscription = new Map<
	string,
	{
		eventChannel: string;
		listener: (_event: Electron.IpcRendererEvent, payload: WatcherEvent) => void;
	}
>();

const api: LiteElectronApi = {
	absorptionPlan: (params) =>
		ipcRenderer.invoke("workspace:absorption-plan", params) as Promise<Array<CommitAbsorption>>,
	absorb: (params) => ipcRenderer.invoke("workspace:absorb", params) as Promise<number>,
	apply: (params) => ipcRenderer.invoke("workspace:apply", params) as Promise<ApplyOutcome>,
	onAskpassPrompt: (callback) => {
		const listener = (_event: Electron.IpcRendererEvent, payload: AskpassPromptEvent) => {
			callback(payload);
		};
		ipcRenderer.on("askpass:prompt", listener);
		return () => ipcRenderer.removeListener("askpass:prompt", listener);
	},
	submitAskpassPromptResponse: (params) =>
		ipcRenderer.invoke("askpass:submit-response", params) as Promise<void>,
	assignHunk: (params) => ipcRenderer.invoke("workspace:assign-hunk", params) as Promise<void>,
	branchCheckoutNew: (params) =>
		ipcRenderer.invoke("workspace:branch-checkout-new", params) as Promise<BranchCheckoutResult>,
	branchCreate: (params) =>
		ipcRenderer.invoke("workspace:branch-create", params) as Promise<BranchCreateResult>,
	branchDetails: (params) =>
		ipcRenderer.invoke("workspace:branch-details", params) as Promise<BranchDetails>,
	branchDiff: (params) =>
		ipcRenderer.invoke("workspace:branch-diff", params) as Promise<TreeChanges>,
	changesInWorktree: (projectId) =>
		ipcRenderer.invoke("workspace:changes-in-worktree", projectId) as Promise<WorktreeChanges>,
	clipboardWriteText: (text) =>
		ipcRenderer.invoke("lite:clipboard-write-text", text) as Promise<void>,
	commitAmend: (params) =>
		ipcRenderer.invoke("workspace:commit-amend", params) as Promise<CommitCreateResult>,
	commitCreate: (params) =>
		ipcRenderer.invoke("workspace:commit-create", params) as Promise<CommitCreateResult>,
	commitDiscard: (params) =>
		ipcRenderer.invoke("workspace:commit-discard", params) as Promise<CommitDiscardResult>,
	commitDiscardChanges: (params) =>
		ipcRenderer.invoke("workspace:commit-discard-changes", params) as Promise<MoveChangesResult>,
	commitDetailsWithLineStats: (params) =>
		ipcRenderer.invoke(
			"workspace:commit-details-with-line-stats",
			params,
		) as Promise<CommitDetails>,
	discardWorktreeChanges: (params) =>
		ipcRenderer.invoke("workspace:discard-worktree-changes", params) as Promise<Array<DiffSpec>>,
	commitInsertBlank: (params) =>
		ipcRenderer.invoke("workspace:commit-insert-blank", params) as Promise<CommitInsertBlankResult>,
	commitMove: (params) =>
		ipcRenderer.invoke("workspace:commit-move", params) as Promise<CommitMoveResult>,
	commitSquash: (params) =>
		ipcRenderer.invoke("workspace:commit-squash", params) as Promise<CommitSquashResult>,
	commitReword: (params) =>
		ipcRenderer.invoke("workspace:commit-reword", params) as Promise<CommitRewordResult>,
	commitMoveChangesBetween: (params) =>
		ipcRenderer.invoke(
			"workspace:commit-move-changes-between",
			params,
		) as Promise<MoveChangesResult>,
	commitUncommit: (params) =>
		ipcRenderer.invoke("workspace:commit-uncommit", params) as Promise<UncommitResult>,
	commitUncommitChanges: (params) =>
		ipcRenderer.invoke("workspace:commit-uncommit-changes", params) as Promise<MoveChangesResult>,
	getVersion: () => ipcRenderer.invoke("lite:get-version") as Promise<string>,
	getRedoTargetSnapshot: (params) =>
		ipcRenderer.invoke("workspace:get-redo-target-snapshot", params) as Promise<Snapshot | null>,
	getReview: (params) => ipcRenderer.invoke("workspace:get-review", params) as Promise<ForgeReview>,
	getUndoTargetSnapshot: (params) =>
		ipcRenderer.invoke("workspace:get-undo-target-snapshot", params) as Promise<Snapshot | null>,
	headInfo: (projectId) => ipcRenderer.invoke("workspace:head-info", projectId) as Promise<RefInfo>,
	listBranches: (projectId, filter) =>
		ipcRenderer.invoke("workspace:list-branches", projectId, filter) as Promise<
			Array<BranchListing>
		>,
	listEditors: () => ipcRenderer.invoke("workspace:list-editors") as Promise<Array<Editor>>,
	listProjects: () => ipcRenderer.invoke("projects:list") as Promise<Array<ProjectForFrontend>>,
	listReviewsForBranch: (params) =>
		ipcRenderer.invoke("workspace:list-reviews-for-branch", params) as Promise<Array<ForgeReview>>,
	moveBranch: (params) =>
		ipcRenderer.invoke("workspace:move-branch", params) as Promise<MoveBranchResult>,
	openInEditor: (params) => ipcRenderer.invoke("workspace:open-in-editor", params) as Promise<void>,
	pathJoin: (path, ...paths) =>
		ipcRenderer.invoke("lite:path-join", path, ...paths) as Promise<string>,
	publishReview: (params) =>
		ipcRenderer.invoke("workspace:publish-review", params) as Promise<ForgeReview>,
	updateBranchName: (params) =>
		ipcRenderer.invoke("workspace:update-branch-name", params) as Promise<UpdateBranchNameResult>,
	updateReview: (params) => ipcRenderer.invoke("workspace:update-review", params) as Promise<void>,
	tearOffBranch: (params) =>
		ipcRenderer.invoke("workspace:tear-off-branch", params) as Promise<MoveBranchResult>,
	peelRestoreSnapshot: (params) =>
		ipcRenderer.invoke("workspace:peel-restore-snapshot", params) as Promise<Snapshot | null>,
	pushStack: (params) => ipcRenderer.invoke("workspace:push-stack", params) as Promise<PushResult>,
	removeBranch: (params) => ipcRenderer.invoke("workspace:remove-branch", params) as Promise<void>,
	restoreSnapshotWithKind: (params) =>
		ipcRenderer.invoke("workspace:restore-snapshot-with-kind", params) as Promise<void>,
	showNativeMenu: (params) =>
		ipcRenderer.invoke("lite:show-native-menu", params) as Promise<string | null>,
	treeChangeDiffs: (params) =>
		ipcRenderer.invoke("workspace:tree-change-diffs", params) as Promise<UnifiedPatch | null>,
	unapplyStack: (params) => ipcRenderer.invoke("workspace:unapply-stack", params) as Promise<void>,
	workspaceIntegrateUpstream: (params) =>
		ipcRenderer.invoke("workspace:integrate-upstream", params) as Promise<WorkspaceState>,
	/**
	 * Subscribe to a project.
	 *
	 * This sets up a listener to project updates from the Rust-end.
	 *
	 * **Usage**
	 * It's expected that one window has max one subscription per project, although it is possible to have multiple.
	 * The node-end of the application will deduplicate project watchers (there will only ever be one watcher) but
	 * there is no deduplication in terms of project subscriptions.
	 *
	 * The responsability of subscribing once and correctly unsubscribing to a project is shifted to the UI.
	 *
	 * @param projectId - The ID of the project to subscribe to.
	 * @param callback - The callback function to pass the event information to.
	 * @returns A subscription ID.
	 */
	watcherSubscribe: async (projectId, callback) => {
		const { subscriptionId, eventChannel } = (await ipcRenderer.invoke(
			"workspace:watcher-subscribe",
			{ projectId },
		)) as WatcherSubscribeResult;
		const listener = (_event: Electron.IpcRendererEvent, payload: WatcherEvent) => {
			callback(payload);
		};
		watcherListenerBySubscription.set(subscriptionId, { eventChannel, listener });
		ipcRenderer.on(eventChannel, listener);
		return subscriptionId;
	},
	/**
	 * Stop watching a project.
	 *
	 * Remove the listener for a given subscription channel.
	 * If this is the last subscription to a project, the watcher will stop.
	 * @param subscriptionId
	 */
	watcherUnsubscribe: async (subscriptionId) => {
		const registration = watcherListenerBySubscription.get(subscriptionId);
		if (registration) {
			ipcRenderer.removeListener(registration.eventChannel, registration.listener);
			watcherListenerBySubscription.delete(subscriptionId);
		}
		return ipcRenderer.invoke("workspace:watcher-unsubscribe", {
			subscriptionId,
		}) as Promise<boolean>;
	},
	/**
	 * Stop all watchers.
	 *
	 * Remove all subscription listners and stop all watchers.
	 */
	watcherStopAll: async () => {
		for (const { eventChannel, listener } of watcherListenerBySubscription.values())
			ipcRenderer.removeListener(eventChannel, listener);

		watcherListenerBySubscription.clear();
		return ipcRenderer.invoke("workspace:watcher-stop-all") as Promise<number>;
	},
	onUpdateDownloaded: (callback) => {
		const listener = (_event: Electron.IpcRendererEvent, info: UpdateDownloadedEvent) =>
			callback(info);
		ipcRenderer.on("updater:update-downloaded", listener);
		return () => ipcRenderer.removeListener("updater:update-downloaded", listener);
	},
	quitAndInstallUpdate: () => ipcRenderer.invoke("updater:quit-and-install") as Promise<void>,
	platform: process.platform,
};

contextBridge.exposeInMainWorld("lite", api);

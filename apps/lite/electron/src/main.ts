import {
	liteIpcChannels,
	type AssignHunkParams,
	type BranchDetailsParams,
	type BranchDiffParams,
	type CommitAmendParams,
	type CommitCreateParams,
	type CommitDetailsWithLineStatsParams,
	type CommitInsertBlankParams,
	type CommitMoveParams,
	type CommitMoveToBranchParams,
	type CommitRewordParams,
	type CommitMoveChangesBetweenParams,
	type CommitUncommitChangesParams,
	type TreeChangeDiffParams,
	type ApplyParams,
	type UnapplyStackParams,
	type WatcherSubscribeParams,
	type WatcherUnsubscribeParams,
} from "#electron/ipc";
import {
	applyNapi,
	assignHunkNapi,
	branchDetailsNapi,
	branchDiffNapi,
	changesInWorktreeNapi,
	commitAmendNapi,
	commitCreateNapi,
	commitInsertBlankNapi,
	commitRewordNapi,
	commitUncommitChangesNapi,
	headInfoNapi,
	commitMoveNapi,
	commitMoveToBranchNapi,
	commitDetailsWithLineStatsNapi,
	commitMoveChangesBetweenNapi,
	listBranchesNapi,
	listProjectsStatelessNapi,
	treeChangeDiffsNapi,
	unapplyStackNapi,
	watcherStartNapi,
	BranchListingFilter,
	type WatcherHandleNapi,
	type WatcherEvent,
} from "@gitbutler/but-sdk";
import { app, BrowserWindow, ipcMain } from "electron";
import { REACT_DEVELOPER_TOOLS, installExtension } from "electron-devtools-installer";
import { randomUUID } from "node:crypto";
import path from "node:path";
import { fileURLToPath } from "node:url";

const currentFilePath = fileURLToPath(import.meta.url);
const currentDirPath = path.dirname(currentFilePath);

type ProjectWatcherState = {
	handle: WatcherHandleNapi;
	subscriptionIds: Set<string>;
};

type WatcherSubscription = {
	projectId: string;
	sender: Electron.WebContents;
	senderId: number;
	eventChannel: string;
};

const projectWatchers = new Map<string, ProjectWatcherState>();
const pendingProjectWatchers = new Map<string, Promise<ProjectWatcherState>>();
const watcherSubscriptions = new Map<string, WatcherSubscription>();
const senderSubscriptions = new Map<number, Set<string>>();

function removeSubscription(subscriptionId: string): boolean {
	const subscription = watcherSubscriptions.get(subscriptionId);
	if (!subscription) return false;

	watcherSubscriptions.delete(subscriptionId);

	const senderIds = senderSubscriptions.get(subscription.senderId);
	if (senderIds) {
		senderIds.delete(subscriptionId);
		if (senderIds.size === 0) senderSubscriptions.delete(subscription.senderId);
	}

	const projectWatcher = projectWatchers.get(subscription.projectId);
	if (projectWatcher) {
		projectWatcher.subscriptionIds.delete(subscriptionId);
		if (projectWatcher.subscriptionIds.size === 0) {
			try {
				projectWatcher.handle.stop();
			} catch (error) {
				// oxlint-disable-next-line no-console
				console.warn("Failed to stop project watcher", error);
			}
			projectWatchers.delete(subscription.projectId);
		}
	}

	return true;
}

function removeSenderSubscriptions(senderId: number): void {
	const subscriptionIds = senderSubscriptions.get(senderId);
	if (!subscriptionIds) return;

	for (const subscriptionId of subscriptionIds) removeSubscription(subscriptionId);

	senderSubscriptions.delete(senderId);
}

function forwardWatcherEvent(projectId: string, event: WatcherEvent): void {
	const projectWatcher = projectWatchers.get(projectId);
	if (!projectWatcher) return;

	const deadSubscriptions: Array<string> = [];
	for (const subscriptionId of projectWatcher.subscriptionIds) {
		const subscription = watcherSubscriptions.get(subscriptionId);
		if (!subscription || subscription.sender.isDestroyed()) {
			deadSubscriptions.push(subscriptionId);
			continue;
		}

		try {
			subscription.sender.send(subscription.eventChannel, event);
		} catch (sendError) {
			// oxlint-disable-next-line no-console
			console.warn("Failed to forward watcher event to renderer", sendError);
			deadSubscriptions.push(subscriptionId);
		}
	}

	for (const subscriptionId of deadSubscriptions) removeSubscription(subscriptionId);
}

async function ensureProjectWatcher(projectId: string): Promise<ProjectWatcherState> {
	const existing = projectWatchers.get(projectId);
	if (existing) return existing;

	const pending = pendingProjectWatchers.get(projectId);
	if (pending) return pending;

	const creation = Promise.resolve(
		watcherStartNapi(projectId, (err, event) => {
			if (err) {
				// oxlint-disable-next-line no-console
				console.warn("Watcher callback failed", err);
				return;
			}
			forwardWatcherEvent(projectId, event);
		}),
	)
		.then((handle) => {
			const watcherState: ProjectWatcherState = {
				handle,
				subscriptionIds: new Set(),
			};
			projectWatchers.set(projectId, watcherState);
			return watcherState;
		})
		.finally(() => {
			pendingProjectWatchers.delete(projectId);
		});

	pendingProjectWatchers.set(projectId, creation);
	return creation;
}

function stopAllWatchersForShutdown(): number {
	const stopped = watcherSubscriptions.size;

	for (const [projectId, projectWatcher] of projectWatchers)
		try {
			projectWatcher.handle.stop();
		} catch (error) {
			// oxlint-disable-next-line no-console
			console.warn(`Failed to stop project watcher for ${projectId}`, error);
		}

	pendingProjectWatchers.clear();
	projectWatchers.clear();
	watcherSubscriptions.clear();
	senderSubscriptions.clear();

	return stopped;
}

function registerSenderCleanup(sender: Electron.WebContents): void {
	const senderId = sender.id;
	if (senderSubscriptions.has(senderId)) return;

	senderSubscriptions.set(senderId, new Set());
	sender.once("destroyed", () => {
		removeSenderSubscriptions(senderId);
	});
}

function addSenderSubscription(senderId: number, subscriptionId: string): void {
	const subscriptions = senderSubscriptions.get(senderId);
	if (!subscriptions) {
		senderSubscriptions.set(senderId, new Set([subscriptionId]));
		return;
	}

	subscriptions.add(subscriptionId);
}

function stopAllWatchersForShutdownSafely(): void {
	try {
		stopAllWatchersForShutdown();
	} catch (error) {
		// oxlint-disable-next-line no-console
		console.warn("Failed to stop project watchers during shutdown", error);
	}
}

function registerIpcHandlers(): void {
	ipcMain.handle(liteIpcChannels.apply, (_e, { projectId, existingBranch }: ApplyParams) =>
		applyNapi(projectId, existingBranch),
	);
	ipcMain.handle(liteIpcChannels.assignHunk, (_e, { projectId, assignments }: AssignHunkParams) =>
		assignHunkNapi(projectId, assignments),
	);
	ipcMain.handle(
		liteIpcChannels.branchDetails,
		(_e, { projectId, branchName, remote }: BranchDetailsParams) =>
			branchDetailsNapi(projectId, branchName, remote),
	);
	ipcMain.handle(liteIpcChannels.branchDiff, (_e, { projectId, branch }: BranchDiffParams) =>
		branchDiffNapi(projectId, branch),
	);
	ipcMain.handle(liteIpcChannels.changesInWorktree, (_e, projectId: string) =>
		changesInWorktreeNapi(projectId),
	);
	ipcMain.handle(
		liteIpcChannels.commitAmend,
		(_e, { projectId, commitId, changes }: CommitAmendParams) =>
			commitAmendNapi(projectId, commitId, changes),
	);
	ipcMain.handle(
		liteIpcChannels.commitCreate,
		(_e, { projectId, relativeTo, side, changes, message }: CommitCreateParams) =>
			commitCreateNapi(projectId, relativeTo, side, changes, message),
	);
	ipcMain.handle(
		liteIpcChannels.commitDetailsWithLineStats,
		(_e, { projectId, commitId }: CommitDetailsWithLineStatsParams) =>
			commitDetailsWithLineStatsNapi(projectId, commitId),
	);
	ipcMain.handle(
		liteIpcChannels.commitInsertBlank,
		(_e, { projectId, relativeTo, side }: CommitInsertBlankParams) =>
			commitInsertBlankNapi(projectId, relativeTo, side),
	);
	ipcMain.handle(
		liteIpcChannels.commitMove,
		(_e, { projectId, subjectCommitId, anchorCommitId, side }: CommitMoveParams) =>
			commitMoveNapi(projectId, subjectCommitId, anchorCommitId, side),
	);
	ipcMain.handle(
		liteIpcChannels.commitMoveToBranch,
		(_e, { projectId, subjectCommitId, anchorRef }: CommitMoveToBranchParams) =>
			commitMoveToBranchNapi(projectId, subjectCommitId, anchorRef),
	);
	ipcMain.handle(
		liteIpcChannels.commitReword,
		(_e, { projectId, commitId, message }: CommitRewordParams) =>
			commitRewordNapi(projectId, commitId, message),
	);
	ipcMain.handle(
		liteIpcChannels.commitMoveChangesBetween,
		(
			_e,
			{ projectId, sourceCommitId, destinationCommitId, changes }: CommitMoveChangesBetweenParams,
		) => commitMoveChangesBetweenNapi(projectId, sourceCommitId, destinationCommitId, changes),
	);
	ipcMain.handle(
		liteIpcChannels.commitUncommitChanges,
		(_e, { projectId, commitId, changes, assignTo }: CommitUncommitChangesParams) =>
			commitUncommitChangesNapi(projectId, commitId, changes, assignTo),
	);
	ipcMain.handle(liteIpcChannels.getVersion, () => Promise.resolve(app.getVersion()));
	ipcMain.handle(liteIpcChannels.headInfo, (_e, projectId: string) => headInfoNapi(projectId));
	ipcMain.handle(
		liteIpcChannels.listBranches,
		(_e, projectId: string, filter: BranchListingFilter | null) =>
			listBranchesNapi(projectId, filter),
	);
	ipcMain.handle(liteIpcChannels.listProjects, () => listProjectsStatelessNapi());
	ipcMain.handle(liteIpcChannels.ping, (_event, input: string) =>
		Promise.resolve(`pong: ${input}`),
	);
	ipcMain.handle(
		liteIpcChannels.treeChangeDiffs,
		(_e, { projectId, change }: TreeChangeDiffParams) => treeChangeDiffsNapi(projectId, change),
	);
	ipcMain.handle(liteIpcChannels.unapplyStack, (_e, { projectId, stackId }: UnapplyStackParams) =>
		unapplyStackNapi(projectId, stackId),
	);
	ipcMain.handle(
		liteIpcChannels.watcherSubscribe,
		async (event, { projectId }: WatcherSubscribeParams) => {
			const projectWatcher = await ensureProjectWatcher(projectId);
			registerSenderCleanup(event.sender);

			const subscriptionId = randomUUID();
			const eventChannel = `workspace:watcher-event:${randomUUID()}`;

			watcherSubscriptions.set(subscriptionId, {
				projectId,
				sender: event.sender,
				senderId: event.sender.id,
				eventChannel,
			});
			projectWatcher.subscriptionIds.add(subscriptionId);
			addSenderSubscription(event.sender.id, subscriptionId);

			return { subscriptionId, eventChannel };
		},
	);
	ipcMain.handle(
		liteIpcChannels.watcherUnsubscribe,
		(_e, { subscriptionId }: WatcherUnsubscribeParams) => removeSubscription(subscriptionId),
	);
	ipcMain.handle(liteIpcChannels.watcherStopAll, () => stopAllWatchersForShutdown());
}

async function createMainWindow(): Promise<void> {
	const mainWindow = new BrowserWindow({
		width: 1024,
		height: 768,
		webPreferences: {
			contextIsolation: true,
			nodeIntegration: false,
			preload: path.join(currentDirPath, "preload.cjs"),
		},
	});

	mainWindow.maximize();

	const devServerUrl = process.env.VITE_DEV_SERVER_URL;
	if (devServerUrl !== undefined) {
		await mainWindow.loadURL(devServerUrl);
		mainWindow.webContents.openDevTools({ mode: "bottom" });
		return;
	}

	await mainWindow.loadFile(path.join(currentDirPath, "../ui/index.html"));
}

void app.whenReady().then(async () => {
	if (!app.isPackaged) await installExtension(REACT_DEVELOPER_TOOLS);
	registerIpcHandlers();
	await createMainWindow();

	app.on("activate", () => {
		if (BrowserWindow.getAllWindows().length === 0) void createMainWindow();
	});
});

app.on("before-quit", () => {
	stopAllWatchersForShutdownSafely();
});

app.on("window-all-closed", () => {
	stopAllWatchersForShutdownSafely();
	if (process.platform !== "darwin") app.quit();
});

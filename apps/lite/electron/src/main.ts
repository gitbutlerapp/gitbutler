import WatcherManager from "./watcher.js";
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
	type CommitRewordParams,
	type CommitMoveChangesBetweenParams,
	type CommitUncommitChangesParams,
	type MoveBranchParams,
	type TreeChangeDiffParams,
	type ApplyParams,
	type UnapplyStackParams,
	WatcherSubscribeParams,
	WatcherUnsubscribeParams,
} from "./ipc.js";
import {
	apply,
	assignHunk,
	branchDetails,
	branchDiff,
	changesInWorktree,
	commitAmend,
	commitCreate,
	commitInsertBlank,
	commitReword,
	commitUncommitChanges,
	headInfo,
	commitMove,
	commitDetailsWithLineStats,
	commitMoveChangesBetween,
	listBranches,
	listProjectsStateless,
	moveBranch,
	treeChangeDiffs,
	unapplyStack,
	BranchListingFilter,
} from "@gitbutler/but-sdk";
import { app, BrowserWindow, ipcMain } from "electron";
import { REACT_DEVELOPER_TOOLS, installExtension } from "electron-devtools-installer";
import path from "node:path";
import { fileURLToPath } from "node:url";

const currentFilePath = fileURLToPath(import.meta.url);
const currentDirPath = path.dirname(currentFilePath);

function registerIpcHandlers(): void {
	ipcMain.handle(liteIpcChannels.apply, (_e, { projectId, existingBranch }: ApplyParams) =>
		apply(projectId, existingBranch),
	);
	ipcMain.handle(liteIpcChannels.assignHunk, (_e, { projectId, assignments }: AssignHunkParams) =>
		assignHunk(projectId, assignments),
	);
	ipcMain.handle(
		liteIpcChannels.branchDetails,
		(_e, { projectId, branchName, remote }: BranchDetailsParams) =>
			branchDetails(projectId, branchName, remote),
	);
	ipcMain.handle(liteIpcChannels.branchDiff, (_e, { projectId, branch }: BranchDiffParams) =>
		branchDiff(projectId, branch),
	);
	ipcMain.handle(liteIpcChannels.changesInWorktree, (_e, projectId: string) =>
		changesInWorktree(projectId),
	);
	ipcMain.handle(
		liteIpcChannels.commitAmend,
		(_e, { projectId, commitId, changes }: CommitAmendParams) =>
			commitAmend(projectId, commitId, changes),
	);
	ipcMain.handle(
		liteIpcChannels.commitCreate,
		(_e, { projectId, relativeTo, side, changes, message }: CommitCreateParams) =>
			commitCreate(projectId, relativeTo, side, changes, message),
	);
	ipcMain.handle(
		liteIpcChannels.commitDetailsWithLineStats,
		(_e, { projectId, commitId }: CommitDetailsWithLineStatsParams) =>
			commitDetailsWithLineStats(projectId, commitId),
	);
	ipcMain.handle(
		liteIpcChannels.commitInsertBlank,
		(_e, { projectId, relativeTo, side }: CommitInsertBlankParams) =>
			commitInsertBlank(projectId, relativeTo, side),
	);
	ipcMain.handle(
		liteIpcChannels.commitMove,
		(_e, { projectId, subjectCommitId, relativeTo, side }: CommitMoveParams) =>
			commitMove(projectId, subjectCommitId, relativeTo, side),
	);
	ipcMain.handle(
		liteIpcChannels.commitReword,
		(_e, { projectId, commitId, message }: CommitRewordParams) =>
			commitReword(projectId, commitId, message),
	);
	ipcMain.handle(
		liteIpcChannels.commitMoveChangesBetween,
		(
			_e,
			{ projectId, sourceCommitId, destinationCommitId, changes }: CommitMoveChangesBetweenParams,
		) => commitMoveChangesBetween(projectId, sourceCommitId, destinationCommitId, changes),
	);
	ipcMain.handle(
		liteIpcChannels.commitUncommitChanges,
		(_e, { projectId, commitId, changes, assignTo }: CommitUncommitChangesParams) =>
			commitUncommitChanges(projectId, commitId, changes, assignTo),
	);
	ipcMain.handle(liteIpcChannels.getVersion, () => Promise.resolve(app.getVersion()));
	ipcMain.handle(liteIpcChannels.headInfo, (_e, projectId: string) => headInfo(projectId));
	ipcMain.handle(
		liteIpcChannels.listBranches,
		(_e, projectId: string, filter: BranchListingFilter | null) => listBranches(projectId, filter),
	);
	ipcMain.handle(liteIpcChannels.listProjects, () => listProjectsStateless());
	ipcMain.handle(
		liteIpcChannels.moveBranch,
		(_e, { projectId, subjectBranch, targetBranch }: MoveBranchParams) =>
			moveBranch(projectId, subjectBranch, targetBranch),
	);
	ipcMain.handle(liteIpcChannels.ping, (_event, input: string) =>
		Promise.resolve(`pong: ${input}`),
	);
	ipcMain.handle(
		liteIpcChannels.treeChangeDiffs,
		(_e, { projectId, change }: TreeChangeDiffParams) => treeChangeDiffs(projectId, change),
	);
	ipcMain.handle(liteIpcChannels.unapplyStack, (_e, { projectId, stackId }: UnapplyStackParams) =>
		unapplyStack(projectId, stackId),
	);
	ipcMain.handle(
		liteIpcChannels.watcherSubscribe,
		async (event, { projectId }: WatcherSubscribeParams) =>
			WatcherManager.getInstance().subscribeToProject(projectId, event),
	);
	ipcMain.handle(
		liteIpcChannels.watcherUnsubscribe,
		(_e, { subscriptionId }: WatcherUnsubscribeParams) =>
			WatcherManager.getInstance().removeSubscription(subscriptionId),
	);
	ipcMain.handle(liteIpcChannels.watcherStopAll, () =>
		WatcherManager.getInstance().stopAllWatchersForShutdown(),
	);
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
	WatcherManager.getInstance().destroy();
});

app.on("window-all-closed", () => {
	WatcherManager.getInstance().destroy();
	if (process.platform !== "darwin") app.quit();
});

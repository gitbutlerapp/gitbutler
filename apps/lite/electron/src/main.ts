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

	const devServerUrl = process.env.VITE_DEV_SERVER_URL;
	if (devServerUrl !== undefined) {
		await mainWindow.loadURL(devServerUrl);
		mainWindow.webContents.openDevTools({ mode: "detach" });
		return;
	}

	await mainWindow.loadFile(path.join(currentDirPath, "../ui/index.html"));
}

void app.whenReady().then(async () => {
	// TODO: if (!app.isPackaged)
	await installExtension(REACT_DEVELOPER_TOOLS);
	registerIpcHandlers();
	await createMainWindow();

	app.on("activate", () => {
		if (BrowserWindow.getAllWindows().length === 0) void createMainWindow();
	});
});

app.on("window-all-closed", () => {
	if (process.platform !== "darwin") app.quit();
});

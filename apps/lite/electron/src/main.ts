import { checkForUpdates, registerUpdater } from "./updater.js";
import WatcherManager from "./watcher.js";
import {
	liteIpcChannels,
	type AbsorbParams,
	type AbsorptionPlanParams,
	type AssignHunkParams,
	type BranchDetailsParams,
	type BranchDiffParams,
	type CommitAmendParams,
	type CommitCreateParams,
	type CommitDiscardParams,
	type CommitDetailsWithLineStatsParams,
	type CommitInsertBlankParams,
	type CommitMoveParams,
	type CommitSquashParams,
	type CommitRewordParams,
	type CommitMoveChangesBetweenParams,
	type CommitUncommitChangesParams,
	type MoveBranchParams,
	type PushStackLegacyParams,
	type TearOffBranchParams,
	type TreeChangeDiffParams,
	type UpdateBranchNameParams,
	type ApplyParams,
	type ShowNativeMenuParams,
	type UnapplyStackParams,
	WatcherSubscribeParams,
	WatcherUnsubscribeParams,
	NativeMenuPopupItem,
} from "./ipc.js";
import {
	absorb,
	absorptionPlan,
	apply,
	assignHunk,
	branchDetails,
	branchDiff,
	changesInWorktree,
	commitAmend,
	commitCreate,
	commitDiscard,
	commitInsertBlank,
	commitSquash,
	commitReword,
	commitUncommitChanges,
	headInfo,
	commitMove,
	commitDetailsWithLineStats,
	commitMoveChangesBetween,
	listBranches,
	listProjectsStateless,
	moveBranch,
	pushStackLegacy,
	tearOffBranch,
	treeChangeDiffs,
	unapplyStack,
	updateBranchName,
	BranchListingFilter,
} from "@gitbutler/but-sdk";
import { app, BrowserWindow, ipcMain, Menu, type MenuItemConstructorOptions } from "electron";
import { REACT_DEVELOPER_TOOLS, installExtension } from "electron-devtools-installer";
import path from "node:path";
import { fileURLToPath } from "node:url";

const currentFilePath = fileURLToPath(import.meta.url);
const currentDirPath = path.dirname(currentFilePath);

const buildNativeMenuTemplate = (
	items: Array<NativeMenuPopupItem>,
	onItem: (itemId: string) => void,
): Array<MenuItemConstructorOptions> =>
	items.map((item): MenuItemConstructorOptions => {
		if (item._tag === "Separator") return { type: "separator" };
		const itemId = item.itemId;

		return {
			label: item.label,
			enabled: item.enabled,
			click: itemId !== undefined ? () => onItem(itemId) : undefined,
			submenu: item.submenu ? buildNativeMenuTemplate(item.submenu, onItem) : undefined,
		};
	});

const registerIpcHandlers = (): void => {
	ipcMain.handle(
		liteIpcChannels.absorptionPlan,
		(_e, { projectId, target }: AbsorptionPlanParams) => absorptionPlan(projectId, target),
	);
	ipcMain.handle(liteIpcChannels.absorb, (_e, { projectId, absorptionPlan }: AbsorbParams) =>
		absorb(projectId, absorptionPlan),
	);
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
		(_e, { projectId, commitId, changes, dryRun }: CommitAmendParams) =>
			commitAmend(projectId, commitId, changes, dryRun),
	);
	ipcMain.handle(
		liteIpcChannels.commitCreate,
		(_e, { projectId, relativeTo, side, changes, message, dryRun }: CommitCreateParams) =>
			commitCreate(projectId, relativeTo, side, changes, message, dryRun),
	);
	ipcMain.handle(
		liteIpcChannels.commitDiscard,
		(_e, { projectId, subjectCommitId, dryRun }: CommitDiscardParams) =>
			commitDiscard(projectId, subjectCommitId, dryRun),
	);
	ipcMain.handle(
		liteIpcChannels.commitDetailsWithLineStats,
		(_e, { projectId, commitId }: CommitDetailsWithLineStatsParams) =>
			commitDetailsWithLineStats(projectId, commitId),
	);
	ipcMain.handle(
		liteIpcChannels.commitInsertBlank,
		(_e, { projectId, relativeTo, side, dryRun }: CommitInsertBlankParams) =>
			commitInsertBlank(projectId, relativeTo, side, dryRun),
	);
	ipcMain.handle(
		liteIpcChannels.commitMove,
		(_e, { projectId, subjectCommitIds, relativeTo, side, dryRun }: CommitMoveParams) =>
			commitMove(projectId, subjectCommitIds, relativeTo, side, dryRun),
	);
	ipcMain.handle(
		liteIpcChannels.commitSquash,
		(_e, { projectId, sourceCommitId, destinationCommitId, dryRun }: CommitSquashParams) =>
			commitSquash(projectId, sourceCommitId, destinationCommitId, dryRun),
	);
	ipcMain.handle(
		liteIpcChannels.commitReword,
		(_e, { projectId, commitId, message, dryRun }: CommitRewordParams) =>
			commitReword(projectId, commitId, message, dryRun),
	);
	ipcMain.handle(
		liteIpcChannels.commitMoveChangesBetween,
		(
			_e,
			{
				projectId,
				sourceCommitId,
				destinationCommitId,
				changes,
				dryRun,
			}: CommitMoveChangesBetweenParams,
		) => commitMoveChangesBetween(projectId, sourceCommitId, destinationCommitId, changes, dryRun),
	);
	ipcMain.handle(
		liteIpcChannels.commitUncommitChanges,
		(_e, { projectId, commitId, changes, assignTo, dryRun }: CommitUncommitChangesParams) =>
			commitUncommitChanges(projectId, commitId, changes, assignTo, dryRun),
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
		(_e, { projectId, subjectBranch, targetBranch, dryRun }: MoveBranchParams) =>
			moveBranch(projectId, subjectBranch, targetBranch, dryRun),
	);
	ipcMain.handle(
		liteIpcChannels.updateBranchName,
		(_e, { projectId, stackId, branchName, newName }: UpdateBranchNameParams) =>
			updateBranchName(projectId, stackId, branchName, newName),
	);
	ipcMain.handle(
		liteIpcChannels.tearOffBranch,
		(_e, { projectId, subjectBranch, dryRun }: TearOffBranchParams) =>
			tearOffBranch(projectId, subjectBranch, dryRun),
	);
	ipcMain.handle(liteIpcChannels.ping, (_event, input: string) =>
		Promise.resolve(`pong: ${input}`),
	);
	ipcMain.handle(
		liteIpcChannels.pushStackLegacy,
		(_e, { projectId, stackId, branch }: PushStackLegacyParams) =>
			pushStackLegacy(projectId, stackId, false, false, branch, true),
	);
	ipcMain.handle(
		liteIpcChannels.showNativeMenu,
		async (event, { items, position }: ShowNativeMenuParams) => {
			const window = BrowserWindow.fromWebContents(event.sender);
			if (!window) return null;

			let selectedItemId: string | null = null;
			const menu = Menu.buildFromTemplate(
				buildNativeMenuTemplate(items, (itemId) => {
					selectedItemId = itemId;
				}),
			);

			await new Promise<void>((resolve) => {
				menu.popup({
					window,
					x: Math.round(position.x),
					y: Math.round(position.y),
					callback: () => resolve(),
				});
			});

			return selectedItemId;
		},
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
};

const createMainWindow = async (): Promise<void> => {
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
		mainWindow.webContents.openDevTools({ mode: "bottom" });
		return;
	}

	await mainWindow.loadFile(path.join(currentDirPath, "../ui/index.html"));
	registerUpdater(mainWindow);
	checkForUpdates();
};

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

import { app, BrowserWindow, ipcMain } from "electron";
import electronUpdater, { type AppUpdater, type UpdateDownloadedEvent } from "electron-updater";
import { liteIpcChannels } from "./ipc.js";
import { env } from "node:process";

let updaterWindow: BrowserWindow | null = null;
let updaterRegistered = false;

const getAutoUpdater = (): AppUpdater => {
	const { autoUpdater } = electronUpdater;
	return autoUpdater;
};

const sendUpdateDownloaded = (event: UpdateDownloadedEvent): void => {
	if (!updaterWindow || updaterWindow.isDestroyed()) return;
	updaterWindow.webContents.send(liteIpcChannels.updaterUpdateDownloaded, event);
};

export const registerUpdater = (mainWindow: BrowserWindow): void => {
	updaterWindow = mainWindow;
	if (updaterRegistered) return;
	updaterRegistered = true;

	const autoUpdater = getAutoUpdater();
	autoUpdater.autoDownload = true;
	autoUpdater.autoInstallOnAppQuit = true;
	autoUpdater.on("update-downloaded", (event) => sendUpdateDownloaded(event));
	autoUpdater.on("error", (error) => {
		// oxlint-disable-next-line no-console
		console.error("Update error", error);
	});

	ipcMain.handle(liteIpcChannels.updaterQuitAndInstall, () => {
		autoUpdater.quitAndInstall(false);
	});
};

export const checkForUpdates = (): void => {
	const updater = getAutoUpdater();

	if (
		!app.isPackaged ||
		env.LITE_NO_AUTOUPDATE === "1" ||
		process.platform === "win32" ||
		updater.currentVersion.prerelease.includes("dev")
	)
		return;

	void updater.checkForUpdates().catch((error) => {
		// oxlint-disable-next-line no-console
		console.error("Failed to check for updates", error);
	});
};

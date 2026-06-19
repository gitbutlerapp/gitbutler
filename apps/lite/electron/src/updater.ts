import { app, BrowserWindow, dialog } from "electron";
import electronUpdater, { type AppUpdater, type UpdateDownloadedEvent } from "electron-updater";
import { env } from "node:process";

let updaterWindow: BrowserWindow | null = null;
let updaterRegistered = false;
let updateDialogShown = false;

const getAutoUpdater = (): AppUpdater => {
	const { autoUpdater } = electronUpdater;
	return autoUpdater;
};

const showUpdateDownloadedDialog = async (event: UpdateDownloadedEvent): Promise<void> => {
	if (updateDialogShown) return;
	if (!updaterWindow || updaterWindow.isDestroyed()) return;

	updateDialogShown = true;

	const { response } = await dialog.showMessageBox(updaterWindow, {
		type: "info",
		buttons: ["Restart and install"],
		defaultId: 0,
		cancelId: 0,
		message: `Update ${event.version} downloaded`,
		detail: "Restart GitButler to install the update.",
	});

	if (response === 0) getAutoUpdater().quitAndInstall(false);
};

export const registerUpdater = (mainWindow: BrowserWindow): void => {
	updaterWindow = mainWindow;
	if (updaterRegistered) return;
	updaterRegistered = true;

	const autoUpdater = getAutoUpdater();
	autoUpdater.autoDownload = true;
	autoUpdater.autoInstallOnAppQuit = true;
	autoUpdater.on("update-downloaded", (event) => {
		void showUpdateDownloadedDialog(event).catch((error) => {
			// oxlint-disable-next-line no-console
			console.error("Failed to show update dialog", error);
		});
	});
	autoUpdater.on("error", (error) => {
		// oxlint-disable-next-line no-console
		console.error("Update error", error);
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

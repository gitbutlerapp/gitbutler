import { app, type BrowserWindow, dialog } from "electron";
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
		// Escape resolves to `cancelId`, so without a second button dismissing the dialog
		// would restart the app mid-work rather than close the notice.
		buttons: ["Restart and install", "Later"],
		defaultId: 0,
		cancelId: 1,
		message: `Update ${event.version} downloaded`,
		detail: "Restart GitButler to install the update, or keep working and it installs on quit.",
	});

	if (response === 0) getAutoUpdater().quitAndInstall(false);
};

export const registerUpdater = (mainWindow: BrowserWindow): void => {
	updaterWindow = mainWindow;
	if (updaterRegistered) return;
	updaterRegistered = true;

	const autoUpdater = getAutoUpdater();
	autoUpdater.autoDownload = autoUpdateEnabled;
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

/** Mirrors the `autoUpdate` setting, so a check can be refused without unregistering. */
let autoUpdateEnabled = true;

export const setAutoUpdateEnabled = (enabled: boolean): void => {
	autoUpdateEnabled = enabled;
	if (!updaterRegistered) return;
	// Only affects a download that has not started; one already downloaded still
	// installs on quit, which is what electron-updater has already committed to.
	getAutoUpdater().autoDownload = enabled;
};

export const checkForUpdates = (): void => {
	const updater = getAutoUpdater();

	if (
		!autoUpdateEnabled ||
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

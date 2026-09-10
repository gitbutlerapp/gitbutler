/** @file This module is stateful as a wrapper around the stateful electron-updater. */

import { app, autoUpdater as nativeUpdater, BrowserWindow } from "electron";
import { autoUpdater, type ProgressInfo } from "electron-updater";
import { once } from "node:events";
import { shutdownMetrics } from "./metrics.js";
import {
	canDownloadUpdate,
	type AvailabilitySnapshot,
	type InstallationStatus,
} from "./updater-state.js";

autoUpdater.autoDownload = false;

let status: InstallationStatus = { _tag: "Idle" };

// The library's downloadUpdate() accepts no version; it uses the last available check's target.
// Mirror that version so we can reject stale download requests.
let candidateVersion: string | undefined;

export const getUpdateStatus = (): InstallationStatus => status;

const setUpdateStatusAndNotify = (next: InstallationStatus): void => {
	status = next;

	for (const window of BrowserWindow.getAllWindows())
		window.webContents.send("updateStatusChange", next);
};

export const checkForUpdates = async (): Promise<AvailabilitySnapshot> => {
	if (!app.isPackaged || autoUpdater.currentVersion.prerelease.includes("dev"))
		return { _tag: "Unavailable" };

	const res = await autoUpdater.checkForUpdates();
	if (!res) return { _tag: "Unavailable" };

	const {
		isUpdateAvailable,
		updateInfo: { version },
	} = res;
	if (!isUpdateAvailable) return { _tag: "UpToDate" };

	candidateVersion = version;
	return { _tag: "Available", version };
};

export const downloadUpdate = async (version: string): Promise<void> => {
	if (!canDownloadUpdate(status)) throw new Error("An update is already in progress.");
	if (candidateVersion !== version)
		throw new Error("The available update changed. Check for updates and try again.");

	setUpdateStatusAndNotify({ _tag: "Downloading", version });

	const onProgress = (progress: ProgressInfo) =>
		setUpdateStatusAndNotify({
			_tag: "Downloading",
			version,
			progress,
		});
	autoUpdater.on("download-progress", onProgress);
	const ac = new AbortController();
	try {
		// On macOS, native preparation continues after download completion.
		//
		// The library's readiness flag can still refer to an older version, so wait for the new version
		// to be prepared before allowing a restart.
		const prepared =
			process.platform === "darwin"
				? once(nativeUpdater, "update-downloaded", { signal: ac.signal })
				: undefined;

		await Promise.all([prepared, autoUpdater.downloadUpdate()]);

		setUpdateStatusAndNotify({ _tag: "Ready", version });
	} catch (error) {
		setUpdateStatusAndNotify({ _tag: "Idle" });

		throw error;
	} finally {
		autoUpdater.removeListener("download-progress", onProgress);
		ac.abort();
	}
};

/** Success exits the app and will never resolve, however failure may still reject. */
export const installUpdate = async (): Promise<never> => {
	if (status._tag !== "Ready") throw new Error("No update is ready to install.");

	setUpdateStatusAndNotify({ _tag: "Installing", version: status.version });

	// Flush before quitting so the metrics handler doesn't interrupt the updater's restart.
	await shutdownMetrics();

	const { promise, reject } = Promise.withResolvers<never>();

	const onError = (error: unknown, context?: string) => {
		// electron-updater emits all failures through the same error channel, ignore irrelevant ones.
		if (context?.startsWith("Cannot check for updates:")) return;

		reject(error);
	};

	try {
		autoUpdater.on("error", onError);
		autoUpdater.quitAndInstall();

		return await promise;
	} catch (error) {
		setUpdateStatusAndNotify({ _tag: "Idle" });

		throw error;
	} finally {
		autoUpdater.removeListener("error", onError);
	}
};

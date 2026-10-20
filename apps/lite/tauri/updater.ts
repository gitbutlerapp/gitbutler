import type { AppSettings } from "@gitbutler/but-sdk";
import { getVersion } from "@tauri-apps/api/app";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type DownloadEvent, type Update } from "@tauri-apps/plugin-updater";
import {
	canDownloadUpdate,
	type AvailabilitySnapshot,
	type InstallationStatus,
} from "#electron/updater-state.ts";
import { backend, type Subscribe } from "./backend.ts";

let status: InstallationStatus = { _tag: "Idle" };
let candidate: Update | undefined;
let downloaded: Update | undefined;
const listeners = new Set<Parameters<Subscribe>[0]>();

const setStatus = (next: InstallationStatus): void => {
	status = next;
	for (const listener of listeners) listener(next);
};

// Local builds are versioned 0.0.0. Builds with `disable-auto-updates` (package-managed installs)
// zero the check interval and can't replace themselves.
const updatesUnavailable = async (): Promise<boolean> => {
	if ((await getVersion()) === "0.0.0") return true;
	const settings = (await backend("get_app_settings")) as AppSettings;
	return settings.ui.checkForUpdatesIntervalInSeconds === 0;
};

// Tauri reports every chunk; only announce whole-percent steps.
const reportProgress = (version: string) => {
	const startedAt = Date.now();
	let total = 0;
	let transferred = 0;
	let reported = 0;
	return (event: DownloadEvent): void => {
		if (event.event === "Started") total = event.data.contentLength ?? 0;
		if (event.event !== "Progress" || total === 0) return;
		transferred += event.data.chunkLength;
		const percent = (transferred / total) * 100;
		if (Math.floor(percent) === Math.floor((reported / total) * 100)) return;
		const elapsedSeconds = Math.max((Date.now() - startedAt) / 1000, 1);
		setStatus({
			_tag: "Downloading",
			version,
			progress: {
				total,
				transferred,
				percent,
				delta: transferred - reported,
				bytesPerSecond: transferred / elapsedSeconds,
			},
		});
		reported = transferred;
	};
};

export const handlers = {
	getUpdateStatus: (): InstallationStatus => status,
	checkForUpdates: async (): Promise<AvailabilitySnapshot> => {
		if (await updatesUnavailable()) return { _tag: "Unavailable" };
		// ponytail: superseded Update resources stay open until reload; close them if checks get frequent.
		const update = await check();
		if (update === null) return { _tag: "UpToDate" };
		candidate = update;
		return { _tag: "Available", version: update.version };
	},
	downloadUpdate: async (version: string): Promise<void> => {
		if (!canDownloadUpdate(status)) throw new Error("An update is already in progress.");
		const update = candidate;
		if (update?.version !== version)
			throw new Error("The available update changed. Check for updates and try again.");

		setStatus({ _tag: "Downloading", version });
		try {
			await update.download(reportProgress(version));
			downloaded = update;
			setStatus({ _tag: "Ready", version });
		} catch (error) {
			setStatus({ _tag: "Idle" });
			throw error;
		}
	},
	installUpdate: async (): Promise<void> => {
		if (status._tag !== "Ready" || downloaded === undefined)
			throw new Error("No update is ready to install.");

		setStatus({ _tag: "Installing", version: status.version });
		try {
			await downloaded.install();
			await relaunch();
		} catch (error) {
			setStatus({ _tag: "Idle" });
			throw error;
		}
	},
};

export const channels: Record<string, Subscribe> = {
	updateStatusChange: (listener) => {
		listeners.add(listener);
		return () => void listeners.delete(listener);
	},
};

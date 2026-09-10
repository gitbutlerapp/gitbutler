import { afterEach, assert, beforeEach, expect, it, vi } from "vitest";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import path from "node:path";
import type * as ChildProcess from "node:child_process";
import type * as ElectronUpdater from "electron-updater";
import type { DownloadExecutorTask } from "electron-updater/out/AppUpdater.js";
import type { InstallationStatus } from "#electron/updater-state.ts";

const mocks = await vi.hoisted(async () => {
	const { EventEmitter } = await import("node:events");
	return {
		native: new EventEmitter(),
		send: vi.fn<(channel: string, state: InstallationStatus) => void>(),
		updater: Object.assign(new EventEmitter(), {
			currentVersion: { prerelease: [] as Array<string> },
			autoDownload: true,
			autoInstallOnAppQuit: true,
			checkForUpdates: vi.fn(),
			downloadUpdate: vi.fn(),
			quitAndInstall: vi.fn(),
		}),
		shutdownMetrics: vi.fn(),
		reportError: vi.fn(),
	};
});

vi.mock("electron", () => ({
	app: { isPackaged: true },
	autoUpdater: mocks.native,
	BrowserWindow: {
		getAllWindows: () => [{ webContents: { isDestroyed: () => false, send: mocks.send } }],
	},
}));
vi.mock("electron-updater", () => ({ autoUpdater: mocks.updater }));
vi.mock("../../electron/src/metrics.js", () => ({
	shutdownMetrics: mocks.shutdownMetrics,
	reportError: mocks.reportError,
}));

const available = (version = "0.0.201") => ({ isUpdateAvailable: true, updateInfo: { version } });

beforeEach(() => {
	vi.resetModules();
	vi.resetAllMocks();
	vi.spyOn(process, "platform", "get").mockReturnValue("darwin");
	mocks.native.removeAllListeners();
	mocks.updater.removeAllListeners();
	// electron-updater installs its own error logger.
	mocks.updater.on("error", () => {});
	mocks.native.on("error", (error) => mocks.updater.emit("error", error));
	mocks.updater.currentVersion.prerelease = [];
	mocks.updater.checkForUpdates.mockResolvedValue(available());
	mocks.updater.downloadUpdate.mockImplementation(async () => {
		mocks.native.emit("update-downloaded");
		return [];
	});
});

afterEach(() => {
	vi.restoreAllMocks();
});

it("checks without downloading before consent", async () => {
	const api = await import("../../electron/src/updater.js");
	expect(await api.checkForUpdates()).toEqual({ _tag: "Available", version: "0.0.201" });
	expect(mocks.updater.autoDownload).toBe(false);
	expect(mocks.updater.autoInstallOnAppQuit).toBe(true);
	expect(mocks.updater.downloadUpdate).not.toHaveBeenCalled();
	expect(api.getUpdateStatus()).toEqual({ _tag: "Idle" });
	expect(mocks.send).not.toHaveBeenCalled();
});

it("preserves the selected release and installation status when a check fails", async () => {
	const api = await import("../../electron/src/updater.js");
	await api.checkForUpdates();
	mocks.updater.checkForUpdates.mockImplementationOnce(async () => {
		const error = new Error("Offline");
		mocks.updater.emit("error", error);
		throw error;
	});
	await expect(api.checkForUpdates()).rejects.toThrow("Offline");
	expect(api.getUpdateStatus()).toEqual({ _tag: "Idle" });
	expect(mocks.send).not.toHaveBeenCalled();
	expect(mocks.reportError).not.toHaveBeenCalled();
	await api.downloadUpdate("0.0.201");
	expect(api.getUpdateStatus()).toEqual({ _tag: "Ready", version: "0.0.201" });
});

it("rejects installation when no update is ready", async () => {
	const api = await import("../../electron/src/updater.js");
	await expect(api.installUpdate()).rejects.toThrow("No update is ready to install.");
	expect(api.getUpdateStatus()).toEqual({ _tag: "Idle" });
	expect(mocks.shutdownMetrics).not.toHaveBeenCalled();
	expect(mocks.updater.quitAndInstall).not.toHaveBeenCalled();
	expect(mocks.send).not.toHaveBeenCalled();
});

it("keeps a recoverable snapshot and waits for native preparation on macOS", async () => {
	const api = await import("../../electron/src/updater.js");
	await api.checkForUpdates();
	mocks.updater.downloadUpdate.mockResolvedValueOnce([]);
	const downloading = api.downloadUpdate("0.0.201");
	await Promise.resolve();
	expect(api.getUpdateStatus()).toEqual({ _tag: "Downloading", version: "0.0.201" });
	expect(await api.checkForUpdates()).toEqual({ _tag: "Available", version: "0.0.201" });
	expect(mocks.updater.checkForUpdates).toHaveBeenCalledTimes(2);
	await expect(api.installUpdate()).rejects.toThrow("No update is ready to install.");
	expect(mocks.updater.quitAndInstall).not.toHaveBeenCalled();

	mocks.native.emit("update-downloaded");
	await downloading;
	expect(api.getUpdateStatus()).toEqual({ _tag: "Ready", version: "0.0.201" });
	expect(mocks.updater.autoInstallOnAppQuit).toBe(true);
	expect(mocks.send).toHaveBeenLastCalledWith("updateStatusChange", api.getUpdateStatus());
	expect(await api.checkForUpdates()).toEqual({ _tag: "Available", version: "0.0.201" });
});

it("publishes download progress without treating 100 percent as installation readiness", async () => {
	const api = await import("../../electron/src/updater.js");
	await api.checkForUpdates();
	const { promise, resolve } = Promise.withResolvers<Array<string>>();
	mocks.updater.downloadUpdate.mockReturnValueOnce(promise);
	const downloading = api.downloadUpdate("0.0.201");
	expect(mocks.updater.listenerCount("download-progress")).toBe(1);
	const progress = {
		percent: 42,
		transferred: 63_000_000,
		total: 150_000_000,
		delta: 1_000_000,
		bytesPerSecond: 500_000,
	};
	mocks.updater.emit("download-progress", progress);
	expect(api.getUpdateStatus()).toEqual({ _tag: "Downloading", version: "0.0.201", progress });
	expect(mocks.send).toHaveBeenLastCalledWith("updateStatusChange", api.getUpdateStatus());
	mocks.updater.emit("download-progress", {
		...progress,
		percent: 100,
		transferred: progress.total,
	});
	resolve([]);
	await Promise.resolve();
	await expect(api.installUpdate()).rejects.toThrow("No update is ready to install.");
	expect(api.getUpdateStatus()._tag).toBe("Downloading");
	mocks.native.emit("update-downloaded");
	await downloading;
	expect(api.getUpdateStatus()).toEqual({ _tag: "Ready", version: "0.0.201" });
	expect(mocks.updater.listenerCount("download-progress")).toBe(0);
	mocks.updater.emit("download-progress", progress);
	expect(api.getUpdateStatus()._tag).toBe("Ready");
});

it("flushes metrics before restarting and rejects a second install request", async () => {
	const api = await import("../../electron/src/updater.js");
	await api.checkForUpdates();
	await api.downloadUpdate("0.0.201");
	let finish!: () => void;
	mocks.shutdownMetrics.mockReturnValue(
		new Promise<void>((resolve) => {
			finish = resolve;
		}),
	);
	const installing = api.installUpdate();
	await expect(api.installUpdate()).rejects.toThrow("No update is ready to install.");
	expect(api.getUpdateStatus()).toEqual({ _tag: "Installing", version: "0.0.201" });
	expect(mocks.updater.quitAndInstall).not.toHaveBeenCalled();
	finish();
	await Promise.resolve();
	await Promise.resolve();
	const rejected = expect(installing).rejects.toThrow("Installer failed");
	mocks.updater.emit("error", new Error("Installer failed"));
	await rejected;
	expect(mocks.shutdownMetrics).toHaveBeenCalledOnce();
	expect(mocks.updater.quitAndInstall).toHaveBeenCalledExactlyOnceWith();
});

it("rejects and publishes a download failure once when it emits an error and rejects", async () => {
	const api = await import("../../electron/src/updater.js");
	await api.checkForUpdates();
	mocks.updater.downloadUpdate.mockImplementationOnce(async () => {
		const error = new Error("Download failed");
		mocks.updater.emit("error", error);
		throw error;
	});
	await expect(api.downloadUpdate("0.0.201")).rejects.toThrow("Download failed");
	expect(mocks.reportError).not.toHaveBeenCalled();
	expect(api.getUpdateStatus()).toEqual({ _tag: "Idle" });
	expect(mocks.updater.autoInstallOnAppQuit).toBe(true);
	expect(mocks.send.mock.calls.filter(([, state]) => state._tag === "Idle")).toHaveLength(1);
	expect(mocks.native.listenerCount("update-downloaded")).toBe(0);
	expect(mocks.updater.listenerCount("download-progress")).toBe(0);
	await api.checkForUpdates();
	await api.downloadUpdate("0.0.201");
	expect(api.getUpdateStatus()).toEqual({ _tag: "Ready", version: "0.0.201" });
});

it("allows checking again after native preparation fails following the download", async () => {
	const api = await import("../../electron/src/updater.js");
	await api.checkForUpdates();
	mocks.updater.downloadUpdate.mockResolvedValueOnce([]);
	const downloading = api.downloadUpdate("0.0.201");
	await Promise.resolve();
	mocks.native.emit("error", new Error("Native preparation failed"));
	await expect(downloading).rejects.toThrow("Native preparation failed");
	expect(api.getUpdateStatus()._tag).toBe("Idle");
	await api.checkForUpdates();
	expect(mocks.updater.checkForUpdates).toHaveBeenCalledTimes(2);
	await api.downloadUpdate("0.0.201");
	expect(api.getUpdateStatus()).toEqual({ _tag: "Ready", version: "0.0.201" });
});

it("surfaces event-only install failures and allows another check", async () => {
	const api = await import("../../electron/src/updater.js");
	await api.checkForUpdates();
	await api.downloadUpdate("0.0.201");
	mocks.updater.quitAndInstall.mockImplementationOnce(() => {
		mocks.updater.emit("error", new Error("Installer failed"));
	});
	await expect(api.installUpdate()).rejects.toThrow("Installer failed");
	expect(mocks.reportError).not.toHaveBeenCalled();
	expect(api.getUpdateStatus()).toEqual({ _tag: "Idle" });
	expect(mocks.updater.autoInstallOnAppQuit).toBe(true);
	await api.checkForUpdates();
	expect(mocks.updater.checkForUpdates).toHaveBeenCalledTimes(2);
});

it("keeps installation pending and rejects a late failure while publishing its status", async () => {
	const api = await import("../../electron/src/updater.js");
	await api.checkForUpdates();
	await api.downloadUpdate("0.0.201");
	const installing = api.installUpdate();
	const settled = vi.fn();
	void installing.then(settled, settled);
	await Promise.resolve();
	await Promise.resolve();
	expect(mocks.updater.quitAndInstall).toHaveBeenCalledOnce();
	expect(settled).not.toHaveBeenCalled();
	const error = new Error("Installer failed after returning");
	const rejected = expect(installing).rejects.toBe(error);
	mocks.updater.emit("error", error);
	await rejected;
	expect(mocks.reportError).not.toHaveBeenCalled();
	expect(mocks.updater.listenerCount("error")).toBe(1);
	expect(mocks.send).toHaveBeenLastCalledWith("updateStatusChange", { _tag: "Idle" });
});

it("removes the install error listener when starting installation throws", async () => {
	const api = await import("../../electron/src/updater.js");
	await api.checkForUpdates();
	await api.downloadUpdate("0.0.201");
	const error = new Error("Cannot start installer");
	mocks.updater.quitAndInstall.mockImplementationOnce(() => {
		throw error;
	});
	await expect(api.installUpdate()).rejects.toBe(error);
	expect(mocks.updater.listenerCount("error")).toBe(1);
	expect(mocks.reportError).not.toHaveBeenCalled();
});

it("can download the selected release while checking for a newer version", async () => {
	const api = await import("../../electron/src/updater.js");
	await api.checkForUpdates();
	let finish!: (result: ReturnType<typeof available>) => void;
	mocks.updater.checkForUpdates.mockReturnValueOnce(
		new Promise((resolve) => {
			finish = resolve;
		}),
	);
	const checking = api.checkForUpdates();
	await api.downloadUpdate("0.0.201");
	expect(api.getUpdateStatus()).toEqual({ _tag: "Ready", version: "0.0.201" });
	finish(available("0.0.202"));
	await checking;
	expect(api.getUpdateStatus()).toEqual({ _tag: "Ready", version: "0.0.201" });
	await expect(api.downloadUpdate("0.0.201")).rejects.toThrow("available update changed");
	expect(mocks.updater.downloadUpdate).toHaveBeenCalledOnce();
	await api.downloadUpdate("0.0.202");
	expect(api.getUpdateStatus()).toEqual({ _tag: "Ready", version: "0.0.202" });
});

it("does not download a newer release without consent for that version", async () => {
	const api = await import("../../electron/src/updater.js");
	await api.checkForUpdates();
	mocks.updater.checkForUpdates.mockResolvedValue(available("0.0.202"));
	await api.checkForUpdates();
	await expect(api.downloadUpdate("0.0.201")).rejects.toThrow("available update changed");
	expect(mocks.updater.downloadUpdate).not.toHaveBeenCalled();
	expect(mocks.updater.autoInstallOnAppQuit).toBe(true);
});

it.each([
	{ result: { isUpdateAvailable: false, updateInfo: { version: "0.0.200" } }, tag: "UpToDate" },
	{ result: null, tag: "Unavailable" },
])("preserves the selected release when a check returns $tag", async ({ result, tag }) => {
	const api = await import("../../electron/src/updater.js");
	await api.checkForUpdates();
	mocks.updater.checkForUpdates.mockResolvedValueOnce(result);
	expect(await api.checkForUpdates()).toEqual({ _tag: tag });
	await api.downloadUpdate("0.0.201");
	expect(mocks.updater.downloadUpdate).toHaveBeenCalledOnce();
	expect(api.getUpdateStatus()).toEqual({ _tag: "Ready", version: "0.0.201" });
});

it("does not wait for a macOS event on Linux", async () => {
	vi.spyOn(process, "platform", "get").mockReturnValue("linux");
	mocks.updater.downloadUpdate.mockResolvedValueOnce([]);
	const api = await import("../../electron/src/updater.js");
	await api.checkForUpdates();
	await api.downloadUpdate("0.0.201");
	expect(api.getUpdateStatus()).toEqual({ _tag: "Ready", version: "0.0.201" });
});

it("discovers a newer release during a download without changing that download", async () => {
	const api = await import("../../electron/src/updater.js");
	await api.checkForUpdates();
	mocks.updater.downloadUpdate.mockResolvedValueOnce([]);
	const downloading = api.downloadUpdate("0.0.201");
	await Promise.resolve();
	const snapshot = api.getUpdateStatus();
	mocks.updater.checkForUpdates.mockResolvedValueOnce(available("0.0.202"));
	expect(await api.checkForUpdates()).toEqual({ _tag: "Available", version: "0.0.202" });
	expect(api.getUpdateStatus()).toEqual(snapshot);
	await expect(api.downloadUpdate("0.0.202")).rejects.toThrow();
	mocks.native.emit("update-downloaded");
	await downloading;
	expect(mocks.updater.downloadUpdate).toHaveBeenCalledOnce();
});

it("does not fail a download when a concurrent check emits an error", async () => {
	const api = await import("../../electron/src/updater.js");
	await api.checkForUpdates();
	mocks.updater.downloadUpdate.mockResolvedValueOnce([]);
	const downloading = api.downloadUpdate("0.0.201");
	await Promise.resolve();
	const snapshot = api.getUpdateStatus();
	mocks.updater.checkForUpdates.mockImplementationOnce(async () => {
		const error = new Error("Offline");
		mocks.updater.emit("error", error, "Cannot check for updates: Offline");
		throw error;
	});
	await expect(api.checkForUpdates()).rejects.toThrow("Offline");
	expect(api.getUpdateStatus()).toEqual(snapshot);
	mocks.native.emit("update-downloaded");
	await downloading;
});

it("does not reject installation when a concurrent check fails", async () => {
	const api = await import("../../electron/src/updater.js");
	await api.checkForUpdates();
	await api.downloadUpdate("0.0.201");
	const installing = api.installUpdate();
	await Promise.resolve();
	const snapshot = api.getUpdateStatus();
	mocks.updater.checkForUpdates.mockImplementationOnce(async () => {
		const error = new Error("Offline");
		mocks.updater.emit("error", error, "Cannot check for updates: Offline");
		throw error;
	});
	await expect(api.checkForUpdates()).rejects.toThrow("Offline");
	expect(api.getUpdateStatus()).toEqual(snapshot);
	const rejected = expect(installing).rejects.toThrow("Installer failed");
	mocks.updater.emit("error", new Error("Installer failed"));
	await rejected;
});

it("does not restart the real MacUpdater before a replacement update is prepared", async () => {
	const { MacUpdater } = await vi.importActual<typeof ElectronUpdater>("electron-updater");
	const { GenericProvider } = await import("electron-updater/out/providers/GenericProvider.js");
	const dir = await mkdtemp(path.join(tmpdir(), "lite-mac-updater-"));
	const archive = path.join(dir, "update.zip");
	await writeFile(archive, "test update");
	const native = Object.assign(mocks.native, {
		setFeedURL: vi.fn<(feed: { url: string; headers: Record<string, string> }) => void>(),
		checkForUpdates: vi.fn<() => Promise<void>>(),
		quitAndInstall: vi.fn(() => {
			throw new Error("Prevent exiting the test runner");
		}),
	});
	// Consume the real proxy response, but let the test decide when native preparation finishes.
	native.checkForUpdates.mockImplementation(async () => {
		const feed = native.setFeedURL.mock.lastCall?.[0];
		assert(feed);
		const manifest = await fetch(feed.url, { headers: feed.headers });
		const { url } = (await manifest.json()) as { url: string };
		const response = await fetch(url);
		expect(await response.text()).toBe("test update");
	});
	// Skip only artifact acquisition/cache IO; retain MacUpdater's download handoff and restart logic.
	class FixtureMacUpdater extends MacUpdater {
		protected override async executeDownload(task: DownloadExecutorTask): Promise<Array<string>> {
			await task.done?.({
				...task.downloadUpdateOptions.updateInfoAndProvider.info,
				downloadedFile: archive,
			});
			return [archive];
		}
	}
	// Vitest's module mocks do not intercept the library's CommonJS require("electron").
	const require = createRequire(import.meta.url);
	const childProcess = require("node:child_process") as typeof ChildProcess;
	vi.spyOn(childProcess, "execFileSync").mockReturnValue("");
	const electronExports = require("electron") as unknown;
	const electronModule = require.cache[require.resolve("electron")];
	assert(electronModule);
	let updater: FixtureMacUpdater;
	try {
		electronModule.exports = { autoUpdater: native };
		updater = new FixtureMacUpdater(
			{ provider: "generic", url: "https://updates.invalid/" },
			{
				version: "0.0.200",
				name: "test",
				isPackaged: true,
				appUpdateConfigPath: path.join(dir, "app-update.yml"),
				userDataPath: dir,
				baseCachePath: dir,
				whenReady: async () => {},
				relaunch: vi.fn(),
				quit: vi.fn(),
				onQuit: vi.fn(),
			},
		);
	} finally {
		electronModule.exports = electronExports;
	}
	updater.autoDownload = false;
	updater.disableDifferentialDownload = true;
	updater.logger = null;
	const check = vi.spyOn(GenericProvider.prototype, "getLatestVersion");
	const release = (version: string) => ({
		version,
		releaseDate: "2026-09-14T00:00:00Z",
		path: `${version}.zip`,
		sha512: "unused",
		files: [{ url: `https://updates.invalid/${version}.zip`, sha512: "unused", size: 11 }],
	});
	mocks.updater.checkForUpdates.mockImplementation(() => updater.checkForUpdates());
	mocks.updater.downloadUpdate.mockImplementation(() => updater.downloadUpdate());
	mocks.updater.quitAndInstall.mockImplementation(() => updater.quitAndInstall());
	try {
		const api = await import("../../electron/src/updater.js");
		check.mockResolvedValue(release("0.0.201"));
		await api.checkForUpdates();
		const first = api.downloadUpdate("0.0.201");
		await vi.waitFor(() => expect(native.checkForUpdates).toHaveBeenCalledTimes(1));
		await native.checkForUpdates.mock.results[0]?.value;
		native.emit("update-downloaded");
		await first;

		check.mockResolvedValue(release("0.0.202"));
		await api.checkForUpdates();
		const second = api.downloadUpdate("0.0.202");
		await vi.waitFor(() => expect(native.checkForUpdates).toHaveBeenCalledTimes(2));
		await native.checkForUpdates.mock.results[1]?.value;
		await vi.waitFor(() =>
			expect(mocks.updater.downloadUpdate.mock.settledResults[1]?.type).toBe("fulfilled"),
		);
		await expect(api.installUpdate()).rejects.toThrow("No update is ready to install.");
		expect(native.quitAndInstall).not.toHaveBeenCalled();

		native.emit("update-downloaded");
		await second;
		await expect(api.installUpdate()).rejects.toThrow("Prevent exiting the test runner");
		expect(native.quitAndInstall).toHaveBeenCalledOnce();
	} finally {
		// Close the real proxy through the library; app.quit is mocked.
		updater.autoRunAppAfterInstall = false;
		native.emit("update-downloaded");
		updater.quitAndInstall();
		await rm(dir, { recursive: true, force: true });
	}
});

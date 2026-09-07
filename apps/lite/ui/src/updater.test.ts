import { afterEach, expect, it, vi } from "vitest";
import type { BrowserWindow } from "electron";

const updater = vi.hoisted(() => ({
	currentVersion: { prerelease: [] },
	on: vi.fn(),
	checkForUpdates: vi.fn().mockResolvedValue(null),
}));

vi.mock("electron", () => ({ app: { isPackaged: true }, dialog: {} }));
vi.mock("electron-updater", () => ({ default: { autoUpdater: updater } }));
vi.mock("../../electron/src/metrics.js", () => ({ shutdownMetrics: vi.fn() }));

afterEach(() => {
	vi.clearAllTimers();
	vi.useRealTimers();
	vi.unstubAllEnvs();
	vi.restoreAllMocks();
});

it("checks hourly without duplicate timers when another window registers", async () => {
	vi.useFakeTimers();
	vi.stubEnv("LITE_NO_AUTOUPDATE", "0");
	vi.spyOn(process, "platform", "get").mockReturnValue("darwin");
	const { registerUpdater } = await import("../../electron/src/updater.js");
	registerUpdater({} as BrowserWindow);
	registerUpdater({} as BrowserWindow);

	const hour = 60 * 60 * 1000;
	await vi.advanceTimersByTimeAsync(hour - 1);
	expect(updater.checkForUpdates).not.toHaveBeenCalled();
	await vi.advanceTimersByTimeAsync(1);
	expect(updater.checkForUpdates).toHaveBeenCalledTimes(1);
	await vi.advanceTimersByTimeAsync(hour);
	expect(updater.checkForUpdates).toHaveBeenCalledTimes(2);
});

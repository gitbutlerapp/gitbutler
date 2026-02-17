import { EventContext } from "$lib/analytics/eventContext";
import { PostHogWrapper } from "$lib/analytics/posthog";
import { type Update } from "$lib/backend";
import { ShortcutService } from "$lib/shortcuts/shortcutService";
import { mockCreateBackend } from "$lib/testing/mockBackend";
import { getSettingsdServiceMock } from "$lib/testing/mockSettingsdService";
import { UpdaterService } from "$lib/updater/updater";
import { get } from "svelte/store";
import { expect, test, describe, vi, beforeEach, afterEach } from "vitest";

/**
 * It is important to understand the sync `get` method performs a store subscription
 * under the hood.
 */
describe("Updater", () => {
	let updater: UpdaterService;
	const backend = mockCreateBackend();
	const MockSettingsService = getSettingsdServiceMock();
	const shortcuts = new ShortcutService(backend);
	const settingsService = new MockSettingsService();
	const eventContext = new EventContext();
	const posthog = new PostHogWrapper(settingsService, backend, eventContext);
	const updateIntervalMs = 3600 * 1000;

	beforeEach(() => {
		vi.useFakeTimers();
		updater = new UpdaterService(backend, posthog, shortcuts, updateIntervalMs);
		vi.spyOn(backend, "listen").mockReturnValue(async () => {});
	});

	afterEach(() => {
		vi.restoreAllMocks();
		vi.clearAllTimers();
	});

	test("should not show up-to-date on interval check", async () => {
		vi.spyOn(backend, "checkUpdate").mockReturnValue(mockUpdate(null));
		await updater.checkForUpdate();
		expect(get(updater.update)).toMatchObject({});
	});

	test("should show up-to-date on manual check", async () => {
		vi.spyOn(backend, "checkUpdate").mockReturnValue(mockUpdate(null));
		await updater.checkForUpdate(true); // manual = true;
		expect(get(updater.update)).toHaveProperty("status", "Up-to-date");
	});

	test("should prompt again on new version", async () => {
		const body = "release notes";

		vi.spyOn(backend, "checkUpdate").mockReturnValue(
			mockUpdate({
				version: "1",
				body,
			}),
		);

		await updater.checkForUpdate();
		const update1 = get(updater.update);
		expect(update1).toHaveProperty("version", "1");
		expect(update1).toHaveProperty("releaseNotes", body);
		updater.dismiss();

		vi.spyOn(backend, "checkUpdate").mockReturnValue(
			mockUpdate({
				version: "2",
				body,
			}),
		);
		await updater.checkForUpdate();
		const update2 = get(updater.update);
		expect(update2).toHaveProperty("version", "2");
		expect(update2).toHaveProperty("releaseNotes", body);
	});

	test("should not prompt download for seen version", async () => {
		const version = "1";
		const body = "release notes";

		vi.spyOn(backend, "checkUpdate").mockReturnValue(
			mockUpdate({
				version,
				body,
			}),
		);
		await updater.checkForUpdate();

		const update1 = get(updater.update);
		expect(update1).toHaveProperty("version", version);
		expect(update1).toHaveProperty("releaseNotes", body);

		updater.dismiss();
		await updater.checkForUpdate();
		const update2 = get(updater.update);
		expect(update2).toMatchObject({});
	});

	test("should check for updates continously", async () => {
		const mock = vi.spyOn(backend, "checkUpdate").mockReturnValue(mockUpdate(null));

		const unsubscribe = updater.update.subscribe(() => {});
		expect(mock).toHaveBeenCalledOnce();

		for (let i = 2; i < 12; i++) {
			await vi.advanceTimersByTimeAsync(updateIntervalMs);
			expect(mock).toHaveBeenCalledTimes(i);
		}
		unsubscribe();
	});

	test("should respect disableAutoChecks setting", async () => {
		const mock = vi.spyOn(backend, "checkUpdate").mockReturnValue(mockUpdate(null));

		// Set disableAutoChecks to true
		updater.disableAutoChecks.set(true);

		// Try to check for updates (should be skipped when disabled)
		await updater.checkForUpdate();
		expect(mock).not.toHaveBeenCalled();

		// Set disableAutoChecks to false
		updater.disableAutoChecks.set(false);

		// Try to check for updates (should work when enabled)
		await updater.checkForUpdate();
		expect(mock).toHaveBeenCalledOnce();
	});

	test("should ignore disableAutoChecks setting when manual update", async () => {
		const mock = vi.spyOn(backend, "checkUpdate").mockReturnValue(mockUpdate(null));
		const manualCheck = true;

		updater.disableAutoChecks.set(true);

		await updater.checkForUpdate(manualCheck);
		expect(mock).toHaveBeenCalledOnce();

		updater.disableAutoChecks.set(false);

		await updater.checkForUpdate(manualCheck);
		expect(mock).toHaveBeenCalledTimes(2);
	});

	test("should disable updater when updateIntervalMs is 0", async () => {
		const mock = vi.spyOn(backend, "checkUpdate").mockReturnValue(mockUpdate(null));

		// Create updater with updateIntervalMs = 0
		const disabledUpdater = new UpdaterService(backend, posthog, shortcuts, 0);

		// Subscribe to the update store (this triggers start())
		const unsubscribe = disabledUpdater.update.subscribe(() => {});

		// checkUpdate should not be called when updateIntervalMs is 0
		expect(mock).not.toHaveBeenCalled();

		// Verify that even after time passes, no automatic checks happen (no timer was set)
		await vi.advanceTimersByTimeAsync(3600 * 1000);
		expect(mock).not.toHaveBeenCalled();

		unsubscribe();
	});
});

async function mockUpdate(update: Partial<Update> | null): Promise<Update | null> {
	if (update === null) {
		return await Promise.resolve(null);
	}

	return await Promise.resolve({
		download: () => {},
		install: () => {},
		...update,
	} as Update);
}

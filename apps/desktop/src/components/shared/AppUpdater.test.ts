import AppUpdater from "$components/shared/AppUpdater.svelte";
import { type Update } from "$lib/backend";
import { ShortcutService } from "$lib/shortcuts/shortcutService";
import { EventContext } from "$lib/telemetry/eventContext";
import { PostHogWrapper } from "$lib/telemetry/posthog";
import { mockCreateBackend } from "$lib/testing/mockBackend";
import { getSettingsdServiceMock } from "$lib/testing/mockSettingsdService";
import { UPDATER_SERVICE, UpdaterService } from "$lib/updater/updater";
import { fireEvent, render, screen } from "@testing-library/svelte";
import { expect, test, describe, vi, beforeEach, afterEach } from "vitest";

describe("AppUpdater", () => {
	let updater: UpdaterService;
	let context: Map<any, any>;
	const backend = mockCreateBackend();
	const shortcuts = new ShortcutService(backend);
	const MockSettingsService = getSettingsdServiceMock();
	const settingsService = new MockSettingsService();
	const eventContext = new EventContext();
	const posthog = new PostHogWrapper(settingsService, backend, eventContext);

	beforeEach(() => {
		vi.useFakeTimers();
		updater = new UpdaterService(backend, posthog, shortcuts, 3600 * 1000);
		context = new Map([[UPDATER_SERVICE._key, updater]]);
		vi.spyOn(backend, "listen").mockReturnValue(async () => {});
		vi.mock("$env/dynamic/public", () => {
			return {
				env: {
					PUBLIC_FLATPAK_ID: undefined,
				},
			};
		});
	});

	afterEach(() => {
		vi.restoreAllMocks();
		vi.clearAllTimers();
	});

	test("should be hidden if no update", async () => {
		vi.spyOn(backend, "checkUpdate").mockReturnValue(mockUpdate(null));

		render(AppUpdater, { context });
		await vi.advanceTimersToNextTimerAsync();

		const updateBanner = screen.queryByTestId("update-banner");
		expect(updateBanner).toBe(null);
	});

	test("should display download button", async () => {
		vi.spyOn(backend, "checkUpdate").mockReturnValue(
			mockUpdate({
				version: "1",
				body: "release notes",
			}),
		);

		render(AppUpdater, { context });
		await vi.advanceTimersToNextTimerAsync();

		const button = screen.getByTestId("download-update");
		expect(button).toBeVisible();
	});

	test("should link to the downloads page for .deb installs", async () => {
		vi.spyOn(backend, "checkUpdate").mockReturnValue(
			mockUpdate({
				version: "1",
				body: "release notes",
			}),
		);
		vi.spyOn(backend, "manualInstall").mockResolvedValue(true);
		const openExternalUrl = vi.spyOn(backend, "openExternalUrl");

		render(AppUpdater, { context });
		await vi.advanceTimersToNextTimerAsync();

		// The updater can't replace a .deb install, so there's nothing to install in place.
		expect(screen.queryByTestId("download-update")).toBe(null);
		await fireEvent.click(screen.getByTestId("download-package"));
		expect(openExternalUrl).toHaveBeenCalledWith("https://app.gitbutler.com/downloads");
	});

	test("should display download button again on manual check after dismiss", async () => {
		vi.spyOn(backend, "checkUpdate").mockReturnValue(
			mockUpdate({
				version: "1",
				body: "release notes",
			}),
		);

		render(AppUpdater, { context });
		await vi.advanceTimersToNextTimerAsync();

		expect(screen.getByTestId("download-update")).toBeVisible();

		updater.dismiss();
		await vi.advanceTimersToNextTimerAsync();
		expect(screen.queryByTestId("update-banner")).toBe(null);

		await updater.checkForUpdate(true);
		await vi.advanceTimersToNextTimerAsync();

		expect(screen.getByTestId("download-update")).toBeVisible();
	});

	test("should display up-to-date on manual check", async () => {
		vi.spyOn(backend, "checkUpdate").mockReturnValue(mockUpdate(null));
		const { getByTestId } = render(AppUpdater, { context });
		await vi.advanceTimersToNextTimerAsync();
		await updater.checkForUpdate(true);

		const button = getByTestId("got-it");
		expect(button).toBeVisible();
	});

	test("should display restart button on install complete", async () => {
		vi.spyOn(backend, "checkUpdate").mockReturnValue(
			mockUpdate({
				version: "2",
				body: "release notes",
			}),
		);

		render(AppUpdater, { context });
		await updater.checkForUpdate(true);
		await vi.runOnlyPendingTimersAsync();
		await updater.downloadAndInstall();
		await vi.runOnlyPendingTimersAsync();

		const button = screen.getByTestId("restart-app");
		expect(button).toBeVisible();
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

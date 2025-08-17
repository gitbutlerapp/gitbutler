import AppUpdater from '$components/AppUpdater.svelte';
import { EventContext } from '$lib/analytics/eventContext';
import { PostHogWrapper } from '$lib/analytics/posthog';
import createBackend, { type Update } from '$lib/backend';
import { ShortcutService } from '$lib/shortcuts/shortcutService';
import { getSettingsdServiceMock } from '$lib/testing/mockSettingsdService';
import { UPDATER_SERVICE, UpdaterService } from '$lib/updater/updater';
import { render, screen } from '@testing-library/svelte';
import { expect, test, describe, vi, beforeEach, afterEach } from 'vitest';

describe('AppUpdater', () => {
	let updater: UpdaterService;
	let context: Map<any, any>;
	const backend = createBackend();
	const shortcuts = new ShortcutService(backend);
	const MockSettingsService = getSettingsdServiceMock();
	const settingsService = new MockSettingsService();
	const eventContext = new EventContext();
	const posthog = new PostHogWrapper(settingsService, backend, eventContext);

	beforeEach(() => {
		vi.useFakeTimers();
		updater = new UpdaterService(backend, posthog, shortcuts);
		context = new Map([[UPDATER_SERVICE._key, updater]]);
		vi.spyOn(backend, 'listen').mockReturnValue(async () => {});
		vi.mock('$env/dynamic/public', () => {
			return {
				env: {
					PUBLIC_FLATPAK_ID: undefined
				}
			};
		});
	});

	afterEach(() => {
		vi.restoreAllMocks();
		vi.clearAllTimers();
	});

	test('should be hidden if no update', async () => {
		vi.spyOn(backend, 'checkUpdate').mockReturnValue(mockUpdate(null));

		render(AppUpdater, { context });
		await vi.advanceTimersToNextTimerAsync();

		const updateBanner = screen.queryByTestId('update-banner');
		expect(updateBanner).toBe(null);
	});

	test('should display download button', async () => {
		vi.spyOn(backend, 'checkUpdate').mockReturnValue(
			mockUpdate({
				version: '1',
				body: 'release notes'
			})
		);

		render(AppUpdater, { context });
		await vi.advanceTimersToNextTimerAsync();

		const button = screen.getByTestId('download-update');
		expect(button).toBeVisible();
	});

	test('should display up-to-date on manual check', async () => {
		vi.spyOn(backend, 'checkUpdate').mockReturnValue(mockUpdate(null));
		const { getByTestId } = render(AppUpdater, { context });
		updater.checkForUpdate(true);
		await vi.advanceTimersToNextTimerAsync();

		const button = getByTestId('got-it');
		expect(button).toBeVisible();
	});

	test('should display restart button on install complete', async () => {
		vi.spyOn(backend, 'checkUpdate').mockReturnValue(
			mockUpdate({
				version: '2',
				body: 'release notes'
			})
		);

		render(AppUpdater, { context });
		await updater.checkForUpdate(true);
		await vi.runOnlyPendingTimersAsync();
		await updater.downloadAndInstall();
		await vi.runOnlyPendingTimersAsync();

		const button = screen.getByTestId('restart-app');
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
		...update
	} as Update);
}

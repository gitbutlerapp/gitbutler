import AppUpdater from './AppUpdater.svelte';
import { Tauri } from '$lib/backend/tauri';
import { UpdaterService } from '$lib/backend/updater';
import { render, screen } from '@testing-library/svelte';
import { expect, test, describe, vi, beforeEach, afterEach } from 'vitest';

describe('AppUpdater', () => {
	let tauri: Tauri;
	let updater: UpdaterService;
	let context: Map<any, any>;

	beforeEach(() => {
		vi.useFakeTimers();
		tauri = new Tauri();
		updater = new UpdaterService(tauri);
		context = new Map([[UpdaterService, updater]]);
		vi.spyOn(tauri, 'listen').mockReturnValue(async () => {});
		vi.spyOn(tauri, 'onUpdaterEvent').mockReturnValue(Promise.resolve(() => {}));
		vi.spyOn(tauri, 'currentVersion').mockReturnValue(Promise.resolve('0.1'));
	});

	afterEach(() => {
		vi.restoreAllMocks();
		vi.clearAllTimers();
	});

	test('should be hidden if no update', async () => {
		vi.spyOn(tauri, 'checkUpdate').mockReturnValue(
			Promise.resolve({
				shouldUpdate: false
			})
		);

		render(AppUpdater, { context });
		await vi.advanceTimersToNextTimerAsync();

		const updateBanner = screen.queryByTestId('update-banner');
		expect(updateBanner).toBeNull();
	});

	test('should display download button', async () => {
		vi.spyOn(tauri, 'checkUpdate').mockReturnValue(
			Promise.resolve({
				shouldUpdate: true,
				manifest: {
					version: '1',
					body: 'release notes',
					date: '2024-01-01'
				}
			})
		);

		render(AppUpdater, { context });
		await vi.advanceTimersToNextTimerAsync();

		const button = screen.getByTestId('download-update');
		expect(button).toBeVisible();
	});

	test('should display up-to-date on manaul check', async () => {
		vi.spyOn(tauri, 'checkUpdate').mockReturnValue(
			Promise.resolve({
				shouldUpdate: false
			})
		);
		render(AppUpdater, { context });
		updater.checkForUpdate(true);
		await vi.advanceTimersToNextTimerAsync();

		const button = screen.getByTestId('got-it');
		expect(button).toBeVisible();
	});

	test('should display restart button on install complete', async () => {
		vi.spyOn(tauri, 'checkUpdate').mockReturnValue(
			Promise.resolve({
				shouldUpdate: true,
				manifest: { version: '1', body: 'release notes', date: '2024-01-01' }
			})
		);
		vi.spyOn(tauri, 'onUpdaterEvent').mockImplementation(async (handler) => {
			handler({ status: 'DONE' });
			return () => {};
		});

		render(AppUpdater, { context });
		updater.checkForUpdate(true);
		await vi.advanceTimersToNextTimerAsync();

		const button = screen.getByTestId('restart-app');
		expect(button).toBeVisible();
	});
});

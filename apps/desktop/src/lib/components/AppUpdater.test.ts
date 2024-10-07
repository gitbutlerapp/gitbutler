import AppUpdater from './AppUpdater.svelte';
import { Tauri } from '$lib/backend/tauri';
import { UpdaterService } from '$lib/backend/updater';
import { render, screen } from '@testing-library/svelte';
import { get } from 'svelte/store';
import { expect, test, describe, vi, beforeEach, afterEach } from 'vitest';
import type { Update } from '@tauri-apps/plugin-updater';

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
		vi.spyOn(tauri, 'currentVersion').mockReturnValue(Promise.resolve('0.1'));
	});

	afterEach(() => {
		vi.restoreAllMocks();
		vi.clearAllTimers();
	});

	test('should be hidden if no update', async () => {
		vi.spyOn(tauri, 'checkUpdate').mockReturnValue(
			Promise.resolve({
				version: '1'
			} as Update)
		);

		render(AppUpdater, { context });
		await vi.advanceTimersToNextTimerAsync();

		const updateBanner = screen.queryByTestId('update-banner');
		expect(updateBanner).toBeNull();
	});

	test('should display download button', async () => {
		vi.spyOn(tauri, 'checkUpdate').mockReturnValue(
			Promise.resolve({
				available: true,
				version: '1',
				body: 'release notes'
			} as Update)
		);

		render(AppUpdater, { context });
		await vi.advanceTimersToNextTimerAsync();

		const button = screen.getByTestId('download-update');
		expect(button).toBeVisible();
	});

	test('should display up-to-date on manaul check', async () => {
		vi.spyOn(tauri, 'checkUpdate').mockReturnValue(
			Promise.resolve({
				available: false
			} as Update)
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
				available: true,
				currentVersion: '1',
				version: '2',
				body: 'release notes',
				download: () => {
					console.log('HELLO');
				},
				install: () => {
					console.log('WORLD');
				}
			} as Update)
		);

		render(AppUpdater, { context });
		await updater.checkForUpdate(true);
		await vi.runOnlyPendingTimersAsync();
		console.log('download and install');
		await updater.downloadAndInstall();
		await vi.runOnlyPendingTimersAsync();
		await vi.advanceTimersToNextTimerAsync();
		await vi.advanceTimersToNextTimerAsync();
		await vi.advanceTimersToNextTimerAsync();
		await vi.advanceTimersToNextTimerAsync();
		await vi.advanceTimersToNextTimerAsync();
		console.log(get(updater.update));

		const button = screen.getByTestId('restart-app');
		expect(button).toBeVisible();
	});
});

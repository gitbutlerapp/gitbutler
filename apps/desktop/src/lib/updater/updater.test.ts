import { UPDATE_INTERVAL_MS, UpdaterService } from './updater';
import { PostHogWrapper } from '$lib/analytics/posthog';
import { Tauri } from '$lib/backend/tauri';
import { get } from 'svelte/store';
import { expect, test, describe, vi, beforeEach, afterEach } from 'vitest';
import type { Update } from '@tauri-apps/plugin-updater';

/**
 * It is important to understand the sync `get` method performs a store subscription
 * under the hood.
 */
describe('Updater', () => {
	let tauri: Tauri;
	let updater: UpdaterService;
	const posthog = new PostHogWrapper();

	beforeEach(() => {
		vi.useFakeTimers();
		tauri = new Tauri();
		updater = new UpdaterService(tauri, posthog);
		vi.spyOn(tauri, 'listen').mockReturnValue(async () => {});
	});

	afterEach(() => {
		vi.restoreAllMocks();
		vi.clearAllTimers();
	});

	test('should not show up-to-date on interval check', async () => {
		vi.spyOn(tauri, 'checkUpdate').mockReturnValue(
			mockUpdate({
				available: false
			})
		);
		await updater.checkForUpdate();
		expect(get(updater.update)).toMatchObject({});
	});

	test('should show up-to-date on manual check', async () => {
		vi.spyOn(tauri, 'checkUpdate').mockReturnValue(
			mockUpdate({
				available: false,
				version: '1'
			})
		);
		await updater.checkForUpdate(true); // manual = true;
		expect(get(updater.update)).toHaveProperty('status', 'Up-to-date');
	});

	test('should prompt again on new version', async () => {
		const body = 'release notes';

		vi.spyOn(tauri, 'checkUpdate').mockReturnValue(
			mockUpdate({
				available: true,
				version: '1',
				body
			})
		);

		await updater.checkForUpdate();
		const update1 = get(updater.update);
		expect(update1).toHaveProperty('version', '1');
		expect(update1).toHaveProperty('releaseNotes', body);
		updater.dismiss();

		vi.spyOn(tauri, 'checkUpdate').mockReturnValue(
			mockUpdate({
				available: true,
				version: '2',
				body
			})
		);
		await updater.checkForUpdate();
		const update2 = get(updater.update);
		expect(update2).toHaveProperty('version', '2');
		expect(update2).toHaveProperty('releaseNotes', body);
	});

	test('should not prompt download for seen version', async () => {
		const version = '1';
		const body = 'release notes';

		vi.spyOn(tauri, 'checkUpdate').mockReturnValue(
			mockUpdate({
				available: true,
				version,
				body
			})
		);
		await updater.checkForUpdate();

		const update1 = get(updater.update);
		expect(update1).toHaveProperty('version', version);
		expect(update1).toHaveProperty('releaseNotes', body);

		updater.dismiss();
		await updater.checkForUpdate();
		const update2 = get(updater.update);
		expect(update2).toMatchObject({});
	});

	test('should check for updates continously', async () => {
		const mock = vi.spyOn(tauri, 'checkUpdate').mockReturnValue(
			mockUpdate({
				available: false
			})
		);

		const unsubscribe = updater.update.subscribe(() => {});
		expect(mock).toHaveBeenCalledOnce();

		for (let i = 2; i < 12; i++) {
			await vi.advanceTimersByTimeAsync(UPDATE_INTERVAL_MS);
			expect(mock).toHaveBeenCalledTimes(i);
		}
		unsubscribe();
	});
});

async function mockUpdate(update: Partial<Update>): Promise<Update> {
	return await Promise.resolve({
		download: () => {},
		install: () => {},
		...update
	} as Update);
}

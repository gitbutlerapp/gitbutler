import { Tauri } from './tauri';
import { UpdaterService } from './updater';
import { get } from 'svelte/store';
import { expect, test, describe, vi, beforeEach, afterEach } from 'vitest';

/**
 * It is important to understand the sync `get` method performs a store subscription
 * under the hood.
 */
describe('Updater', () => {
	let tauri: Tauri;
	let updater: UpdaterService;

	beforeEach(() => {
		vi.useFakeTimers();
		tauri = new Tauri();
		updater = new UpdaterService(tauri);
		vi.spyOn(tauri, 'listen').mockReturnValue(async () => {});
		vi.spyOn(tauri, 'onUpdaterEvent').mockReturnValue(Promise.resolve(() => {}));
		vi.spyOn(tauri, 'currentVersion').mockReturnValue(Promise.resolve('0.1'));
	});

	afterEach(() => {
		vi.restoreAllMocks();
	});

	test('should not show up-to-date on interval check', async () => {
		vi.spyOn(tauri, 'checkUpdate').mockReturnValue(
			Promise.resolve({
				shouldUpdate: false
			})
		);

		await updater.checkForUpdate();
		expect(get(updater.update)).toHaveProperty('status', undefined);
	});

	test('should show up-to-date on manual check', async () => {
		vi.spyOn(tauri, 'checkUpdate').mockReturnValue(
			Promise.resolve({
				shouldUpdate: false
			})
		);
		await updater.checkForUpdate(true); // manual = true;
		expect(get(updater.update)).toHaveProperty('status', 'UPTODATE');
	});

	test('should prompt again on new version', async () => {
		const body = 'release notes';
		const date = '2024-01-01';

		vi.spyOn(tauri, 'checkUpdate').mockReturnValue(
			Promise.resolve({
				shouldUpdate: true,
				manifest: { version: '1', body, date }
			})
		);

		await updater.checkForUpdate();
		const update1 = get(updater.update);
		expect(update1).toHaveProperty('version', '1');
		expect(update1).toHaveProperty('releaseNotes', body);
		updater.dismiss();

		vi.spyOn(tauri, 'checkUpdate').mockReturnValue(
			Promise.resolve({
				shouldUpdate: true,
				manifest: { version: '2', body, date }
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
		const date = '2024-01-01';

		vi.spyOn(tauri, 'checkUpdate').mockReturnValue(
			Promise.resolve({
				shouldUpdate: true,
				manifest: { version, body, date }
			})
		);
		const updater = new UpdaterService(tauri);
		await updater.checkForUpdate();

		const update1 = get(updater.update);
		expect(update1).toHaveProperty('version', version);
		expect(update1).toHaveProperty('releaseNotes', body);

		updater.dismiss();
		await updater.checkForUpdate();
		const update2 = get(updater.update);
		expect(update2).toHaveProperty('version', undefined);
		expect(update2).toHaveProperty('releaseNotes', undefined);
	});
});

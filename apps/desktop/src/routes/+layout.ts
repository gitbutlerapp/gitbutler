import createBackend from '$lib/backend';
import { loadAppSettings } from '$lib/config/appSettings';
import { SettingsService } from '$lib/config/appSettingsV2';
import lscache from 'lscache';
import type { LayoutLoad } from './$types';

// call on startup so we don't accumulate old items
lscache.flushExpired();

export const ssr = false;
export const prerender = false;
export const csr = true;

// eslint-disable-next-line
export const load: LayoutLoad = async () => {
	// Awaited and will block initial render, but it is necessary in order to respect the user
	// settings on telemetry.
	const backend = createBackend();

	const homeDir = await backend.homeDirectory();

	// TODO: Migrate telemetry settings from here to `SettingsService`
	const appSettings = await loadAppSettings(backend);

	// TODO: This should be the only settings service.
	const settingsService = new SettingsService(backend);
	await settingsService.refresh();

	return {
		homeDir,
		backend,
		settingsService,
		appSettings
	};
};

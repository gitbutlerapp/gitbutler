import { initPostHog } from '$lib/analytics/posthog';
import { initSentry } from '$lib/analytics/sentry';
import { AuthService } from '$lib/backend/auth';
import { getCloudApiClient } from '$lib/backend/cloud';
import { ProjectService } from '$lib/backend/projects';
import { UpdaterService } from '$lib/backend/updater';
import { appMetricsEnabled, appErrorReportingEnabled } from '$lib/config/appSettings';
import { UserService } from '$lib/stores/user';
import lscache from 'lscache';
import { config } from 'rxjs';
import type { LayoutLoad } from './$types';

// call on startup so we don't accumulate old items
lscache.flushExpired();

// https://rxjs.dev/api/index/interface/GlobalConfig#properties
config.onUnhandledError = (err) => console.warn(err);

export const ssr = false;
export const prerender = false;
export const csr = true;

let homeDir: () => Promise<string>;

export const load: LayoutLoad = async ({ fetch: realFetch }: { fetch: typeof fetch }) => {
	appErrorReportingEnabled()
		.onDisk()
		.then((enabled) => {
			if (enabled) initSentry();
		});
	appMetricsEnabled()
		.onDisk()
		.then((enabled) => {
			if (enabled) initPostHog();
		});
	const userService = new UserService();

	// TODO: Find a workaround to avoid this dynamic import
	// https://github.com/sveltejs/kit/issues/905
	homeDir = (await import('@tauri-apps/api/path')).homeDir;
	const defaultPath = await homeDir();

	return {
		authService: new AuthService(),
		projectService: new ProjectService(defaultPath),
		cloud: getCloudApiClient({ fetch: realFetch }),
		updaterService: new UpdaterService(),
		userService,
		user$: userService.user$
	};
};

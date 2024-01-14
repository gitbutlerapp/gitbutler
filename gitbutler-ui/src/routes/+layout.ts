import type { LayoutLoad } from './$types';
import { getCloudApiClient } from '$lib/backend/cloud';
import { ProjectService } from '$lib/backend/projects';
import lscache from 'lscache';
import { UpdaterService } from './updater';
import { UserService } from '$lib/stores/user';
import { config } from 'rxjs';
import { initPostHog } from '$lib/analytics/posthog';

// call on startup so we don't accumulate old items
lscache.flushExpired();

// https://rxjs.dev/api/index/interface/GlobalConfig#properties
config.onUnhandledError = (err) => console.warn(err);

export const ssr = false;
export const prerender = false;
export const csr = true;

let homeDir: () => Promise<string>;

export const load: LayoutLoad = async ({ fetch: realFetch }: { fetch: typeof fetch }) => {
	initPostHog();
	const userService = new UserService();
	const updateService = new UpdaterService();

	// TODO: Find a workaround to avoid this dynamic import
	// https://github.com/sveltejs/kit/issues/905
	homeDir = (await import('@tauri-apps/api/path')).homeDir;
	const defaultPath = await homeDir();

	return {
		projectService: new ProjectService(defaultPath),
		cloud: getCloudApiClient({ fetch: realFetch }),
		updateService,
		userService,
		user$: userService.user$
	};
};

import type { LayoutLoad } from './$types';
import { getCloudApiClient } from '$lib/backend/cloud';
import { ProjectService } from '$lib/backend/projects';
import Posthog from '$lib/analytics/posthog';
import Sentry from '$lib/analytics/sentry';
import lscache from 'lscache';
import { newUpdateStore } from './updater';
import { UserService } from '$lib/stores/user';
import { config } from 'rxjs';

// call on startup so we don't accumulate old items
lscache.flushExpired();

// https://rxjs.dev/api/index/interface/GlobalConfig#properties
config.onUnhandledError = (err) => console.warn(err);

export const ssr = false;
export const prerender = false;
export const csr = true;

export const load: LayoutLoad = ({ fetch: realFetch }: { fetch: typeof fetch }) => {
	const posthog = Posthog();
	const sentry = Sentry();
	const userService = new UserService(sentry, posthog);
	return {
		projectService: new ProjectService(),
		cloud: getCloudApiClient({ fetch: realFetch }),
		posthog: posthog,
		sentry: sentry,
		update: newUpdateStore(),
		userService,
		user$: userService.user$
	};
};

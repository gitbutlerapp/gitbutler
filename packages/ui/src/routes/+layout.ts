import type { LayoutLoad } from './$types';
import { getCloudApiClient } from '$lib/backend/cloud';
import { projectsStore } from '$lib/backend/projects';
import Posthog from '$lib/analytics/posthog';
import Sentry from '$lib/analytics/sentry';
import lscache from 'lscache';
import { newUpdateStore } from './updater';

// call on startup so we don't accumulate old items
lscache.flushExpired();

export const ssr = false;
export const prerender = false;
export const csr = true;

export const load: LayoutLoad = ({ fetch: realFetch }: { fetch: typeof fetch }) => ({
	projects: projectsStore,
	cloud: getCloudApiClient({ fetch: realFetch }),
	posthog: Posthog(),
	sentry: Sentry(),
	update: newUpdateStore()
});

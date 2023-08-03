import type { LayoutLoad } from './$types';
import { getCloudApiClient } from '$lib/api/cloud/api';
import { projectsStore } from '$lib/api/ipc/projects';
import Posthog from '$lib/posthog';
import Sentry from '$lib/sentry';

export const ssr = false;
export const prerender = false;
export const csr = true;

export const load: LayoutLoad = ({ fetch: realFetch }: { fetch: typeof fetch }) => ({
	projects: projectsStore,
	cloud: getCloudApiClient({ fetch: realFetch }),
	posthog: Posthog(),
	sentry: Sentry()
});

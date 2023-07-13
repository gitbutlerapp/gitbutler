import type { LayoutLoad } from './$types';
import { api } from '$lib';
import Posthog from '$lib/posthog';
import Sentry from '$lib/sentry';
import { BranchStoresCache } from '$lib/vbranches';
import { loadUserSettings } from '$lib/userSettings';

export const ssr = false;
export const prerender = false;
export const csr = true;

export const load: LayoutLoad = ({ fetch: realFetch }: { fetch: typeof fetch }) => ({
	projects: api.projects.Projects(),
	cloud: api.CloudApi({ fetch: realFetch }),
	branchStoresCache: new BranchStoresCache(),
	posthog: Posthog(),
	sentry: Sentry(),
	userSettings: loadUserSettings()
});

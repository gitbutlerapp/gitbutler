import type { LayoutLoad } from './$types';
import { api, log } from '$lib';
import Posthog from '$lib/posthog';
import Sentry from '$lib/sentry';
import { BranchStoresCache } from '$lib/vbranches';
import { wrapLoadWithSentry } from '@sentry/sveltekit';
import { loadUserSettings } from '$lib/userSettings';

export const ssr = false;
export const prerender = true;
export const csr = true;

export const load: LayoutLoad = wrapLoadWithSentry(({ fetch }) => {
	log.setup();
	return {
		projects: api.projects.Projects(),
		cloud: api.CloudApi({ fetch }),
		branchStoresCache: new BranchStoresCache(),
		posthog: Posthog(),
		sentry: Sentry(),
		userSettings: loadUserSettings()
	};
});

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

import { dev } from '$app/environment';

// It turns out that this function gets called on every navigation when wrapped with Sentry. We
// don't know why it happens, but we should investigate if it happens in prod as well as dev.
// I examined the call stack and found a section suggesting it might not happen in prod.
// TODO(mattias): Investigate and decide what to do
function loadFn({ fetch: realFetch }: { fetch: typeof fetch }) {
	log.setup();
	return {
		projects: api.projects.Projects(),
		cloud: api.CloudApi({ fetch: realFetch }),
		branchStoresCache: new BranchStoresCache(),
		posthog: Posthog(),
		sentry: Sentry(),
		userSettings: loadUserSettings()
	};
}
export const load: LayoutLoad = dev ? loadFn : wrapLoadWithSentry(loadFn);

import type { LayoutLoad } from './$types';
import { api, log } from '$lib';
import Posthog from '$lib/posthog';
import Sentry from '$lib/sentry';
import { wrapLoadWithSentry } from '@sentry/sveltekit';

export const ssr = false;
export const prerender = true;
export const csr = true;

export const load: LayoutLoad = wrapLoadWithSentry(({ fetch }) => {
	log.setup();
	return {
		projects: api.projects.Projects(),
		cloud: api.CloudApi({ fetch }),
		posthog: Posthog(),
		sentry: Sentry()
	};
});

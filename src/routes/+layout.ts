import type { LayoutLoad } from './$types';
import { api, log } from '$lib';
import Posthog from '$lib/posthog';
import Sentry from '$lib/sentry';
import { wrapLoadWithSentry } from '@sentry/sveltekit';
import Events from '$lib/events';
import Hotkeys from '$lib/hotkeys';

export const ssr = false;
export const prerender = true;
export const csr = true;

export const load: LayoutLoad = wrapLoadWithSentry(({ fetch }) => {
	const events = Events();
	log.setup();
	return {
		projects: api.projects.Projects(),
		user: api.users.CurrentUser(),
		cloud: api.CloudApi({ fetch }),
		posthog: Posthog(),
		sentry: Sentry(),
		events,
		hotkeys: Hotkeys(events)
	};
});

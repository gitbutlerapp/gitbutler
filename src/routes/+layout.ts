import { readable } from 'svelte/store';
import type { LayoutLoad } from './$types';
import { building } from '$app/environment';
import type { Project } from '$lib/api';
import { Api } from '$lib/api/cloud';
import Posthog from '$lib/posthog';
import Sentry from '$lib/sentry';
import { setup as setupLogger } from '$lib/log';
import { wrapLoadWithSentry } from '@sentry/sveltekit';
import Events from '$lib/events';
import Hotkeys from '$lib/hotkeys';

export const ssr = false;
export const prerender = true;
export const csr = true;

export const load: LayoutLoad = wrapLoadWithSentry(async ({ fetch }) => {
	const projects = building
		? {
				...readable<Project[]>([]),
				add: (params: { path: string }): Promise<Project> => {
					throw new Error('not implemented');
				},
				get: () => {
					throw new Error('not implemented');
				}
		  }
		: await (await import('$lib/api')).projects.Projects();
	const user = building
		? {
				...readable<undefined>(undefined),
				set: () => {
					throw new Error('not implemented');
				},
				delete: () => {
					throw new Error('not implemented');
				}
		  }
		: await (await import('$lib/api')).users.CurrentUser();
	setupLogger();
	const events = Events();
	return {
		projects,
		user,
		api: Api({ fetch }),
		posthog: Posthog(),
		sentry: Sentry(),
		events,
		hotkeys: Hotkeys(events)
	};
});

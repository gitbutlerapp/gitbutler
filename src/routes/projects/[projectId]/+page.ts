import { building } from '$app/environment';
import type { PageLoad } from './$types';
import { readable } from 'svelte/store';
import type { Activity } from '$lib/api';
import { wrapLoadWithSentry } from '@sentry/sveltekit';

export const load: PageLoad = wrapLoadWithSentry(async ({ params }) => {
	const activity = building
		? readable<Activity[]>([])
		: await import('$lib/api').then((m) =>
				m.git.activities.Activities({ projectId: params.projectId })
		  );
	return {
		activity
	};
});

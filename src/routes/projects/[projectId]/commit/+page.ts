import { building } from '$app/environment';
import { readable } from 'svelte/store';
import type { PageLoad } from '../$types';
import { wrapLoadWithSentry } from '@sentry/sveltekit';

export const load: PageLoad = wrapLoadWithSentry(async ({ parent, params }) => {
	const { project } = await parent();
	const diffs = building
		? readable<Record<string, string>>({})
		: await import('$lib/api').then((m) => m.git.diffs.Diffs({ projectId: params.projectId }));
	return {
		diffs,
		project
	};
});

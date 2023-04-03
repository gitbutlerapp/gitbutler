import { building } from '$app/environment';
import type { PageLoad } from './$types';
import { readable } from 'svelte/store';
import type { Activity } from '$lib/git/activity';

export const load: PageLoad = async ({ params }) => {
	const activity = building
		? readable<Activity[]>([])
		: await import('$lib/git/activity').then((m) => m.default({ projectId: params.projectId }));
	return {
		activity
	};
};

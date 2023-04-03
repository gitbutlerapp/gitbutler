import { building } from '$app/environment';
import { readable } from 'svelte/store';
import type { PageLoad } from '../$types';

export const load: PageLoad = async ({ params }) => {
	const diffs = building
		? readable<Record<string, string>>({})
		: await import('$lib/git/diffs').then((m) => m.default({ projectId: params.projectId }));
	return { diffs };
};

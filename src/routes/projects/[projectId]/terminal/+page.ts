import { readable } from 'svelte/store';
import type { Status } from '$lib/git/statuses';
import { building } from '$app/environment';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({parent, params}) => {
	const statuses = building
		? readable<Status[]>([])
		: await import('$lib/git/statuses').then((m) => m.default({ projectId: params.projectId }));
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
		: await (await import('$lib/users')).default();
	return {
    user,
    statuses
	};
};
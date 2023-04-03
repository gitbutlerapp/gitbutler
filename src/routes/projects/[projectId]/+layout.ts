import { building } from '$app/environment';
import type { Session } from '$lib/sessions';
import type { Status } from '$lib/git/statuses';
import { readable } from 'svelte/store';
import type { LayoutLoad } from './$types';

export const prerender = false;
export const load: LayoutLoad = async ({ parent, params }) => {
	const { projects } = await parent();
	const sessions = building
		? readable<Session[]>([])
		: await import('$lib/sessions').then((m) => m.default({ projectId: params.projectId }));
	const statuses = building
		? readable<Status[]>([])
		: await import('$lib/git/statuses').then((m) => m.default({ projectId: params.projectId }));
	const head = building
		? readable<string>('')
		: await import('$lib/git/head').then((m) => m.default({ projectId: params.projectId }));
	return {
		head,
		statuses,
		sessions,
		project: projects.get(params.projectId),
		projectId: params.projectId
	};
};

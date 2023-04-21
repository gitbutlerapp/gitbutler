import { building } from '$app/environment';
import type { Session } from '$lib/sessions';
import type { Status } from '$lib/git/statuses';
import { readable } from 'svelte/store';
import type { LayoutLoad } from './$types';
import type { Delta } from '$lib/deltas';
import type { Readable } from 'svelte/store';

export const prerender = false;

export const load: LayoutLoad = async ({ parent, params }) => {
	const { projects } = await parent();
	const sessions = building
		? readable<Session[]>([])
		: await import('$lib/sessions').then((m) => m.default({ projectId: params.projectId }));
	const statuses = building
		? readable<Record<string, Status>>({})
		: await import('$lib/git/statuses').then((m) => m.default({ projectId: params.projectId }));
	const head = building
		? readable<string>('')
		: await import('$lib/git/head').then((m) => m.default({ projectId: params.projectId }));
	const deltas = building
		? () => Promise.resolve(readable<Record<string, Delta[]>>({}))
		: (sessionId: string) =>
				import('$lib/deltas').then((m) => m.default({ projectId: params.projectId, sessionId }));

	const cache: Record<string, Promise<Readable<Record<string, Delta[]>>>> = {};
	const cachedDeltas = (sessionId: string) => {
		if (sessionId in cache) {
			return cache[sessionId];
		}
		const promise = deltas(sessionId);
		cache[sessionId] = promise;
		return promise;
	};
	return {
		head,
		statuses,
		sessions,
		project: projects.get(params.projectId),
		projectId: params.projectId,
		getDeltas: cachedDeltas
	};
};

import { building } from '$app/environment';
import type { Session, Delta, Status } from '$lib/api';
import { readable } from 'svelte/store';
import type { LayoutLoad } from './$types';
import type { Readable } from 'svelte/store';

export const prerender = false;

export const load: LayoutLoad = async ({ parent, params }) => {
	const { projects } = await parent();
	const sessions = building
		? readable<Session[]>([])
		: await import('$lib/api').then((m) => m.sessions.Sessions({ projectId: params.projectId }));
	const statuses = building
		? readable<Record<string, Status>>({})
		: await import('$lib/api').then((m) =>
				m.git.statuses.Statuses({ projectId: params.projectId })
		  );
	const head = building
		? readable<string>('')
		: await import('$lib/api').then((m) => m.git.heads.Head({ projectId: params.projectId }));
	const deltas = building
		? () => Promise.resolve(readable<Record<string, Delta[]>>({}))
		: (sessionId: string) =>
				import('$lib/api').then((m) => m.deltas.Deltas({ projectId: params.projectId, sessionId }));
	const diffs = building
		? readable<Record<string, string>>({})
		: await import('$lib/api').then((m) => m.git.diffs.Diffs({ projectId: params.projectId }));

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
		diffs,
		project: projects.get(params.projectId),
		projectId: params.projectId,
		getDeltas: cachedDeltas
	};
};

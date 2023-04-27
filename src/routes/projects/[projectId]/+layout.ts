import type { Delta } from '$lib/api';
import { api } from '$lib';
import type { LayoutLoad } from './$types';
import type { Readable } from 'svelte/store';

export const prerender = false;

export const load: LayoutLoad = async ({ parent, params }) => {
	const { projects } = await parent();

	const deltas = (sessionId: string) =>
		api.deltas.Deltas({ projectId: params.projectId, sessionId });

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
		head: api.git.heads.Head({ projectId: params.projectId }),
		statuses: api.git.statuses.Statuses({ projectId: params.projectId }),
		sessions: api.sessions.Sessions({ projectId: params.projectId }),
		diffs: api.git.diffs.Diffs({ projectId: params.projectId }),
		project: projects.get(params.projectId),
		projectId: params.projectId,
		getDeltas: cachedDeltas
	};
};

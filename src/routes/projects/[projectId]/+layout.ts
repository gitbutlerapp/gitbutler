import { api } from '$lib';
import type { LayoutLoad } from './$types';

export const prerender = false;

export const load: LayoutLoad = async ({ params }) => {
	return {
		head: api.git.heads.Head({ projectId: params.projectId }),
		statuses: api.git.statuses.Statuses({ projectId: params.projectId }),
		sessions: api.sessions.Sessions({ projectId: params.projectId }),
		diffs: api.git.diffs.Diffs({ projectId: params.projectId })
	};
};

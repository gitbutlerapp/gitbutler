import { getHeadStore } from '$lib/backend/heads';
import { getSessionStore } from '$lib/stores/sessions';
import { getDiffsStore } from '$lib/backend/diffs';
import { error } from '@sveltejs/kit';
import type { LayoutLoad } from './$types';
import { getProjectStore } from '$lib/backend/projects';

export const prerender = false;

export const load: LayoutLoad = async ({ params }) => {
	const project = getProjectStore(params.projectId);
	if ((await project.load()) === undefined) throw error(404, new Error('Project not found'));
	return {
		head: getHeadStore(params.projectId),
		sessions: getSessionStore(params.projectId),
		diffs: getDiffsStore({ projectId: params.projectId }),
		project: project
	};
};

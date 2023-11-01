import { getHeadStore } from '$lib/api/heads';
import { getSessionStore } from '$lib/stores/sessions';
import { getDiffsStore } from '$lib/api/diffs';
import { error } from '@sveltejs/kit';
import type { LayoutLoad } from './$types';
import { getProjectStore, type Project } from '$lib/api/projects';

export const prerender = false;

export const load: LayoutLoad = async ({ params }) => {
	const project = getProjectStore({ id: params.projectId });
	if ((await project.load()) === undefined) throw error(404, new Error('Project not found'));
	return {
		head: getHeadStore(params.projectId),
		sessions: getSessionStore(params.projectId),
		diffs: getDiffsStore({ projectId: params.projectId }),
		project: project
	};
};

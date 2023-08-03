import { getHeadStore } from '$lib/api/git/heads';
import { getStatusStore } from '$lib/api/git/statuses';
import { getSessionStore } from '$lib/api/ipc/sessions';
import { getDiffsStore } from '$lib/api/git/diffs';
import { error } from '@sveltejs/kit';
import type { LayoutLoad } from './$types';
import type { Loadable } from '@square/svelte-store';
import { getProjectStore, type Project } from '$lib/api/ipc/projects';

export const prerender = false;

export const load: LayoutLoad = async ({ params }) => {
	const project = getProjectStore({ id: params.projectId });
	if ((await project.load()) === undefined) throw error(404, new Error('Project not found'));
	return {
		head: getHeadStore({ projectId: params.projectId }),
		statuses: getStatusStore({ projectId: params.projectId }),
		sessions: getSessionStore({ projectId: params.projectId }),
		diffs: getDiffsStore({ projectId: params.projectId }),
		project: project as Loadable<Project> & Pick<typeof project, 'update' | 'delete'>
	};
};

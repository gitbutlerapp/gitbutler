import { api } from '$lib';
import { error } from '@sveltejs/kit';
import type { LayoutLoad } from './$types';
import type { Loadable } from '@square/svelte-store';
import type { Project } from '$lib/api';

export const prerender = false;

export const load: LayoutLoad = async ({ params }) => {
	const project = api.projects.Project({ id: params.projectId });
	if ((await project.load()) === undefined) throw error(404, new Error('Project not found'));
	return {
		head: api.git.heads.Head({ projectId: params.projectId }),
		statuses: api.git.statuses.Statuses({ projectId: params.projectId }),
		sessions: api.sessions.Sessions({ projectId: params.projectId }),
		diffs: api.git.diffs.Diffs({ projectId: params.projectId }),
		project: project as Loadable<Project> & Pick<typeof project, 'update' | 'delete'>
	};
};

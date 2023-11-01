import type { PageLoadEvent } from './$types';
import { getProjectStore, type Project } from '$lib/api/ipc/projects';
import type { Loadable } from '@square/svelte-store';

export async function load({ params }: PageLoadEvent) {
	const project = getProjectStore({ id: params.projectId });
	return {
		project: project as Loadable<Project> & Pick<typeof project, 'update' | 'delete'>
	};
}

import type { PageLoadEvent } from './$types';
import { getProjectStore, type Project } from '$lib/backend/projects';
import type { Loadable } from '@square/svelte-store';

export async function load({ params }: PageLoadEvent) {
	const project = getProjectStore(params.projectId);
	return {
		project: project as Loadable<Project>
	};
}

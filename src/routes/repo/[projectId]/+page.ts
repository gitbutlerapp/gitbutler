import type { PageLoadEvent } from './$types';
import { invoke } from '$lib/ipc';
import { getProjectStore, type Project } from '$lib/api/ipc/projects';
import type { Loadable } from '@square/svelte-store';
import type { RemoteBranchName } from '$lib/vbranches/types';

async function getRemoteBranches(params: { projectId: string }) {
	return invoke<Array<RemoteBranchName>>('git_remote_branches', params);
}

export async function load({ params }: PageLoadEvent) {
	const projectId = params.projectId;
	const remoteBranchNames = await getRemoteBranches({ projectId });
	const project = getProjectStore({ id: params.projectId });

	return {
		projectId,
		remoteBranchNames,
		project: project as Loadable<Project> & Pick<typeof project, 'update' | 'delete'>
	};
}

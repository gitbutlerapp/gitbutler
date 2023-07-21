import type { PageLoadEvent } from './$types';
import { invoke } from '$lib/ipc';
import { api } from '$lib';

async function getRemoteBranches(params: { projectId: string }) {
	return invoke<Array<string>>('git_remote_branches', params);
}

export async function load({ parent, params }: PageLoadEvent) {
	const projectId = params.projectId;
	const remoteBranchNames = await getRemoteBranches({ projectId });
	const project = api.projects.get({ id: projectId });

	const { branchStoresCache } = await parent();
	const vbranchStore = branchStoresCache.getVirtualBranchStore(projectId);
	const remoteBranchStore = branchStoresCache.getRemoteBranchStore(projectId);
	const targetBranchStore = branchStoresCache.getBaseBranchStore(projectId);

	return {
		projectId,
		remoteBranchNames,
		project,
		vbranchStore,
		remoteBranchStore,
		targetBranchStore
	};
}

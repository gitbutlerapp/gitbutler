import { plainToInstance } from 'class-transformer';
import { Branch } from './types';
import { invoke } from '$lib/ipc';
import type { PageLoadEvent } from './$types';

async function getVirtualBranches(params: { projectId: string }) {
	return invoke<Array<Branch>>('list_virtual_branches', params);
}

async function getRemoteBranches(params: { projectId: string }) {
	return invoke<Array<string>>('git_remote_branches', params);
}

async function getTargetData(params: { projectId: string }) {
	return invoke<object>('get_target_data', params);
}

function sortBranchHunks(branches: Branch[]): Branch[] {
	for (const branch of branches) {
		for (const file of branch.files) {
			file.hunks.sort((a, b) => b.modifiedAt.getTime() - a.modifiedAt.getTime());
		}
	}
	return branches;
}

export async function load(e: PageLoadEvent) {
	const projectId = e.params.projectId;
	const target = await getTargetData({ projectId });
	const remoteBranches = await getRemoteBranches({ projectId });
	const branches: Branch[] = sortBranchHunks(
		plainToInstance(Branch, await getVirtualBranches({ projectId }))
	);
	return { projectId, target, remoteBranches, branches };
}

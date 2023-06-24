import { plainToInstance } from 'class-transformer';
import { Target, Branch, type BranchData } from './types';
import { invoke } from '$lib/ipc';
import type { PageLoadEvent } from './$types';

async function getVirtualBranches(params: { projectId: string }) {
	return invoke<Array<Branch>>('list_virtual_branches', params);
}

async function getRemoteBranches(params: { projectId: string }) {
	return invoke<Array<string>>('git_remote_branches', params);
}

async function getTargetData(params: { projectId: string }) {
	return invoke<Target>('get_target_data', params);
}

async function getRemoteBranchesData(params: { projectId: string }) {
	return invoke<Array<BranchData>>('git_remote_branches_data', params);
}

function sortBranchHunks(branches: Branch[]): Branch[] {
	for (const branch of branches) {
		for (const file of branch.files) {
			file.hunks.sort((a, b) => b.modifiedAt.getTime() - a.modifiedAt.getTime());
		}
	}
	return branches;
}

function sortBranchData(branchData: BranchData[]): BranchData[] {
	// sort remote_branches_data by date
	return branchData.sort((a, b) => b.lastCommitTs - a.lastCommitTs);
}

export async function load(e: PageLoadEvent) {
	const projectId = e.params.projectId;
	const target = await getTargetData({ projectId });
	const remoteBranches = await getRemoteBranches({ projectId });
	const remoteBranchesData = sortBranchData(await getRemoteBranchesData({ projectId }));
	const branches: Branch[] = sortBranchHunks(
		plainToInstance(Branch, await getVirtualBranches({ projectId }))
	);
	console.log(branches);
	return { projectId, target, remoteBranches, remoteBranchesData, branches };
}

if (import.meta.vitest) {
	const { it, expect } = import.meta.vitest;
	it('sorts by last commit timestamp', () => {
		const bd: BranchData[] = [
			{
				sha: 'a',
				lastCommitTs: 1
			} as BranchData,
			{
				sha: 'b',
				lastCommitTs: 2
			} as BranchData
		];
		expect(sortBranchData(bd)[0].sha).toBe('b');
	});
}

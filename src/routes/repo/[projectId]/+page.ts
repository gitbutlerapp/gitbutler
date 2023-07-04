import type { BranchData } from './types';
import type { PageLoadEvent } from './$types';
import { invoke } from '$lib/ipc';
import { api } from '$lib';

async function getRemoteBranches(params: { projectId: string }) {
	return invoke<Array<string>>('git_remote_branches', params);
}

function sortBranchData(branchData: BranchData[]): BranchData[] {
	// sort remote_branches_data by date
	return branchData.sort((a, b) => b.lastCommitTs - a.lastCommitTs);
}

export async function load(e: PageLoadEvent) {
	const projectId = e.params.projectId;
	const remoteBranchNames = await getRemoteBranches({ projectId });
	const project = api.projects.get({ id: projectId });
	return { projectId, remoteBranchNames, project };
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

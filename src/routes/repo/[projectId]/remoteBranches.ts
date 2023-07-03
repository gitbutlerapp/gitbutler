import { invoke } from '$lib/ipc';
import type { Target, BranchData } from './types';
import { writable, type Loadable, Value } from 'svelte-loadable-store';

import type { Writable, Readable } from '@square/svelte-store';

const cache: Map<string, Readable<Loadable<BranchData[]>>> = new Map();

export interface RemoteBranchOperations {
	setTarget(branch: string): Promise<object>;
}

export function getRemoteBranches(projectId: string): Readable<Loadable<BranchData[]>> {
	const cachedStore = cache.get(projectId);
	if (cachedStore) {
		return cachedStore;
	}
	const writeable = createWriteable(projectId);

	cache.set(projectId, writeable);
	return writeable;
}

function createWriteable(projectId: string) {
	return writable(getRemoteBranchesData(projectId), (set) => {
		setInterval(() => {
			getRemoteBranchesData(projectId).then((branches) => {
				set(sortBranchData(branches));
			});
		}, 60000); // poll since we don't have a way to subscribe to changes
	});
}

function refresh(projectId: string, store: Writable<Loadable<BranchData[]>>) {
	getRemoteBranchesData(projectId).then((newBranches) =>
		store.set({ isLoading: false, value: newBranches })
	);
}

function sortBranchData(branchData: BranchData[]): BranchData[] {
	// sort remote_branches_data by date
	return branchData.sort((a, b) => b.lastCommitTs - a.lastCommitTs);
}

async function getRemoteBranchesData(projectId: string) {
	return invoke<Array<BranchData>>('git_remote_branches_data', { projectId });
}

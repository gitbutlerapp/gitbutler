import { invoke } from '$lib/ipc';
import type { BranchData } from './types';
import { writable, type Loadable } from 'svelte-loadable-store';
import { error } from '$lib/toasts';

import type { Writable, Readable } from '@square/svelte-store';

const cache: Map<string, RemoteBranchOperations & Readable<Loadable<BranchData[]>>> = new Map();

export interface RemoteBranchOperations {
	updateBranchTarget(): Promise<void | object>;
}

export function getRemoteBranches(
	projectId: string
): RemoteBranchOperations & Readable<Loadable<BranchData[]>> {
	const cachedStore = cache.get(projectId);
	if (cachedStore) {
		return cachedStore;
	}
	const writeable = createWriteable(projectId);
	const store: RemoteBranchOperations & Readable<Loadable<BranchData[]>> = {
		subscribe: writeable.subscribe,
		updateBranchTarget: () => updateBranchTarget(writeable, projectId)
	};

	cache.set(projectId, store);
	return store;
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

function updateBranchTarget(writable: Writable<Loadable<BranchData[]>>, projectId: string) {
	return invoke<object>('update_branch_target', { projectId: projectId })
		.then(() => {
			refresh(projectId, writable);
		})
		.catch(() => {
			error('Unable to update target!');
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

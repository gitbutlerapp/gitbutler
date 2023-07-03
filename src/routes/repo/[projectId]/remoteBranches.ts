import { invoke } from '$lib/ipc';
import { git } from '$lib/api/ipc';
import type { BranchData } from './types';
import { writable, type Loadable } from 'svelte-loadable-store';
import { error } from '$lib/toasts';

import type { Writable, Readable } from '@square/svelte-store';

const cache: Map<string, RemoteBranchOperations & Readable<Loadable<BranchData[]>>> = new Map();

export interface RemoteBranchOperations {
	refresh(): Promise<void | object>;
	updateBranchTarget(): Promise<void | object>;
	createvBranchFromBranch(branch: string): Promise<void | string>;
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
		refresh: () => refresh(projectId, writeable),
		subscribe: writeable.subscribe,
		updateBranchTarget: () => updateBranchTarget(writeable, projectId),
		createvBranchFromBranch: (branch) => createvBranchFromBranch(writeable, projectId, branch)
	};

	cache.set(projectId, store);
	return store;
}

function createWriteable(projectId: string) {
	return writable(getRemoteBranchesData(projectId), (set) => {
		git.fetches.subscribe({ projectId }, () => {
			getRemoteBranchesData(projectId).then((branches) => {
				set(sortBranchData(branches));
			});
		});
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
	return getRemoteBranchesData(projectId).then((newBranches) =>
		store.set({ isLoading: false, value: newBranches })
	);
}

function sortBranchData(branchData: BranchData[]): BranchData[] {
	// sort remote_branches_data by date
	return branchData.sort((a, b) => b.lastCommitTs - a.lastCommitTs);
}

async function createvBranchFromBranch(
	writable: Writable<Loadable<BranchData[]>>,
	projectId: string,
	branch: string
) {
	return invoke<string>('create_virtual_branch_from_branch', { projectId, branch })
		.then(() => {
			refresh(projectId, writable);
		})
		.catch(() => {
			error('Unable to create virtual branch from branch!');
		});
}

async function getRemoteBranchesData(projectId: string) {
	return invoke<Array<BranchData>>('git_remote_branches_data', { projectId });
}

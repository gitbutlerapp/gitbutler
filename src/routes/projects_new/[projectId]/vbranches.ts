import { invoke } from '$lib/ipc';
import type { Branch } from './types';
import { stores } from '$lib';
import { Value } from 'svelte-loadable-store';

export function sortBranchHunks(branches: Branch[]): Branch[] {
	for (const branch of branches) {
		for (const file of branch.files) {
			file.hunks.sort((a, b) => b.modifiedAt.getTime() - a.modifiedAt.getTime());
		}
	}
	return branches;
}

async function listVirtualBranches(params: { projectId: string }) {
	return invoke<Array<Branch>>('list_virtual_branches', params);
}

export function getVBranchesOnBackendChange(
	projectId: string,
	callback: (newBranches: Array<Branch>) => void
) {
	stores.sessions({ projectId }).subscribe((sessions) => {
		if (sessions.isLoading) return;
		if (Value.isError(sessions.value)) return;
		const lastSession = sessions.value.at(0);
		if (!lastSession) return;
		return stores
			.deltas({ projectId, sessionId: lastSession.id })
			.subscribe(() =>
				listVirtualBranches({ projectId }).then((newBranches) => callback(newBranches))
			);
	});
}

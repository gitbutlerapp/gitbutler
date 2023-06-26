import { invoke } from '$lib/ipc';
import { Branch } from './types';
import { stores } from '$lib';
import { writable, type Loadable, Value } from 'svelte-loadable-store';
import { plainToInstance } from 'class-transformer';
import type { Readable, Writable } from '@square/svelte-store';

/** Virtual Branch interface with custom operations on top of subscribing */
export interface VirtualBranch extends Readable<Loadable<Branch[]>> {
	/**
	 * Force re-fetch of the branches. Exists temporarily until we have all mutations on this interface.
	 */
	refresh(this: void): void;
	// TODO: The other operations on branchesa, like create, delete, etc.
}

const cache: Record<string, Writable<Loadable<Branch[]>>> = {};

export default (projectId: string): VirtualBranch => {
	if (projectId in cache) {
		const store = cache[projectId];
		return {
			subscribe: store.subscribe,
			refresh: () =>
				list(projectId).then((newBranches) =>
					store.set({ isLoading: false, value: sort(plainToInstance(Branch, newBranches)) })
				)
		};
	}

	// Subscribe to sessions,  grab the last one and subscribe to deltas on it.
	// When a delta comes in, refresh the list of virtual branches.
	const store = writable(list(projectId), (set) => {
		const unsubscribeSessions = stores.sessions({ projectId }).subscribe((sessions) => {
			if (sessions.isLoading) return;
			if (Value.isError(sessions.value)) return;
			const lastSession = sessions.value.at(0);
			if (!lastSession) return;
			const unsubscribeDeltas = stores
				.deltas({ projectId, sessionId: lastSession.id })
				.subscribe(() => {
					list(projectId).then((newBranches) => {
						set(sort(plainToInstance(Branch, newBranches)));
					});
					return () => {
						Promise.resolve(unsubscribeDeltas).then((unsubscribe) => unsubscribe());
					};
				});
			return () => {
				Promise.resolve(unsubscribeSessions).then((unsubscribe) => unsubscribe());
			};
		});
	});
	cache[projectId] = store;

	return {
		subscribe: store.subscribe,
		refresh: () =>
			list(projectId).then((newBranches) =>
				store.set({ isLoading: false, value: sort(plainToInstance(Branch, newBranches)) })
			)
	};
};
function sort(branches: Branch[]): Branch[] {
	for (const branch of branches) {
		for (const file of branch.files) {
			file.hunks.sort((a, b) => b.modifiedAt.getTime() - a.modifiedAt.getTime());
		}
	}
	return branches;
}

async function list(projectId: string): Promise<Branch[]> {
	return invoke<Array<Branch>>('list_virtual_branches', { projectId });
}

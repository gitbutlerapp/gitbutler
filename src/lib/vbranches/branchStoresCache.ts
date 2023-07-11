import { writable, type Loadable, Loaded } from 'svelte-loadable-store';
import type { Readable } from '@square/svelte-store';
import { git } from '$lib/api/ipc';
import { stores } from '$lib';
import type { Branch, BranchData, Target } from './types';
import * as ipc from './ipc';

export interface Refreshable {
	refresh(): Promise<void | object>;
}

export class BranchStoresCache {
	virtualBranchStores: Map<string, Refreshable & Readable<Loadable<Branch[]>>> = new Map();
	remoteBranchStores: Map<string, Refreshable & Readable<Loadable<BranchData[]>>> = new Map();
	targetBranchStores: Map<string, Refreshable & Readable<Loadable<Target>>> = new Map();

	getVirtualBranchStore(projectId: string) {
		const cachedStore = this.virtualBranchStores.get(projectId);
		if (cachedStore) {
			return cachedStore;
		}

		const writableStore = writable(ipc.listVirtualBranches({ projectId }), (set) => {
			stores.sessions({ projectId }).subscribe((sessions) => {
				if (sessions.isLoading) return;
				if (Loaded.isError(sessions)) return;
				const lastSession = sessions.value.at(-1);
				if (!lastSession) return;
				return stores.deltas({ projectId, sessionId: lastSession.id }).subscribe(() => {
					ipc.listVirtualBranches({ projectId }).then(set);
				});
			});
		});
		const refreshableStore = {
			subscribe: writableStore.subscribe,
			refresh: async () => {
				const newBranches = await ipc.listVirtualBranches({ projectId });
				return writableStore.set({ isLoading: false, value: newBranches });
			}
		};
		this.virtualBranchStores.set(projectId, refreshableStore);
		return refreshableStore;
	}

	getRemoteBranchStore(projectId: string) {
		const cachedStore = this.remoteBranchStores.get(projectId);
		if (cachedStore) {
			return cachedStore;
		}
		const writableStore = writable(ipc.getRemoteBranchesData({ projectId }), (set) => {
			git.fetches.subscribe({ projectId }, () => {
				ipc.getRemoteBranchesData({ projectId }).then((branches) => {
					set(sortBranchData(branches));
				});
			});
		});
		const refreshableStore = {
			subscribe: writableStore.subscribe,
			refresh: async () => {
				const newRemoteBranches = await ipc.getRemoteBranchesData({ projectId });
				return writableStore.set({ isLoading: false, value: newRemoteBranches });
			}
		};
		this.remoteBranchStores.set(projectId, refreshableStore);
		return refreshableStore;
	}

	getTargetBranchStore(projectId: string) {
		const cachedStore = this.targetBranchStores.get(projectId);
		if (cachedStore) {
			return cachedStore;
		}
		const writableStore = writable(ipc.getTargetData({ projectId }), (set) => {
			git.fetches.subscribe({ projectId }, () => {
				ipc.getTargetData({ projectId }).then((newTarget) => {
					set(newTarget);
				});
			});
		});
		const refreshableStore = {
			subscribe: writableStore.subscribe,
			refresh: async () => {
				const newTarget = await ipc.getTargetData({ projectId });
				return writableStore.set({ isLoading: false, value: newTarget });
			}
		};
		this.targetBranchStores.set(projectId, refreshableStore);
		return refreshableStore;
	}
}

function sortBranchData(branchData: BranchData[]): BranchData[] {
	return branchData.sort((a, b) => b.lastCommitTs - a.lastCommitTs);
}

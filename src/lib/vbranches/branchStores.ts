import { writable, type Loadable, Value } from 'svelte-loadable-store';
import type { Readable } from '@square/svelte-store';
import { git } from '$lib/api/ipc';
import { stores } from '$lib';
import type { Branch, BranchData, Target } from './types';
import * as ipc from './ipc';

export interface Refreshable {
	refresh(): Promise<void | object>;
}

const virtualBranchStores: Map<string, Refreshable & Readable<Loadable<Branch[]>>> = new Map();
const remoteBranchStores: Map<string, Refreshable & Readable<Loadable<BranchData[]>>> = new Map();
const targetBranchStores: Map<string, Readable<Loadable<Target>>> = new Map();

export function getVirtualBranchStore(projectId: string) {
	const cachedStore = virtualBranchStores.get(projectId);
	if (cachedStore) {
		return cachedStore;
	}
	const writableStore = writable(ipc.listVirtualBranches({ projectId }), (set) => {
		stores.sessions({ projectId }).subscribe((sessions) => {
			if (sessions.isLoading) return;
			if (Value.isError(sessions.value)) return;
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
	virtualBranchStores.set(projectId, refreshableStore);
	return refreshableStore;
}

export function getRemoteBranchStore(projectId: string) {
	const cachedStore = remoteBranchStores.get(projectId);
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
	remoteBranchStores.set(projectId, refreshableStore);
	return refreshableStore;
}
export function getTargetBranchStore(projectId: string) {
	const cachedStore = targetBranchStores.get(projectId);
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
	targetBranchStores.set(projectId, refreshableStore);
	return refreshableStore;
}

function sortBranchData(branchData: BranchData[]): BranchData[] {
	return branchData.sort((a, b) => b.lastCommitTs - a.lastCommitTs);
}

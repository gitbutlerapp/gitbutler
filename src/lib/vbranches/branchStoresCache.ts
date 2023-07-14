import { writable, type Loadable, Loaded } from 'svelte-loadable-store';
import type { Session } from '$lib/api';
import type { Readable } from '@square/svelte-store';
import { git } from '$lib/api/ipc';
import { stores } from '$lib';
import { Target, Branch, BranchData } from './types';
import { plainToInstance } from 'class-transformer';
import { invoke } from '$lib/ipc';

export interface Refreshable {
	refresh(): Promise<void | object>;
}

export class BranchStoresCache {
	virtualBranchStores: Map<string, Refreshable & Readable<Loadable<Branch[]>>> = new Map();
	remoteBranchStores: Map<string, Refreshable & Readable<Loadable<BranchData[]>>> = new Map();
	targetBranchStores: Map<string, Refreshable & Readable<Loadable<Target | null>>> = new Map();

	getVirtualBranchStore(projectId: string) {
		const cachedStore = this.virtualBranchStores.get(projectId);
		if (cachedStore) {
			return cachedStore;
		}

		const writableStore = writable(listVirtualBranches({ projectId }), (set) => {
			stores.sessions({ projectId }).subscribe((sessions) => {
				if (sessions.isLoading) return;
				if (Loaded.isError(sessions)) return;
				const lastSession = sessions.value.at(-1);
				if (!lastSession) return;
				return stores.deltas({ projectId, sessionId: lastSession.id }).subscribe(() => {
					listVirtualBranches({ projectId }).then((newBranches) => {
						branchesWithFileContent(projectId, lastSession.id, newBranches).then((withContent) => {
							set(withContent);
						});
					});
				});
			});
		});
		const refreshableStore = {
			subscribe: writableStore.subscribe,
			refresh: async () => {
				const newBranches = await listVirtualBranches({ projectId });
				const sessions = await invoke<Session[]>('list_sessions', { projectId });
				const lastSession = sessions.at(-1);
				if (!lastSession) {
					return writableStore.set({ isLoading: false, value: newBranches });
				}
				const withContent = await branchesWithFileContent(projectId, lastSession.id, newBranches);
				return writableStore.set({ isLoading: false, value: withContent });
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
		const writableStore = writable(getRemoteBranchesData({ projectId }), (set) => {
			git.fetches.subscribe({ projectId }, () => {
				getRemoteBranchesData({ projectId }).then(set);
			});
		});
		const refreshableStore = {
			subscribe: writableStore.subscribe,
			refresh: async () => {
				const newRemoteBranches = await getRemoteBranchesData({ projectId });
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
		const writableStore = writable(getTargetData({ projectId }), (set) => {
			git.fetches.subscribe({ projectId }, () => {
				getTargetData({ projectId }).then(set);
			});
		});
		const refreshableStore = {
			subscribe: writableStore.subscribe,
			refresh: async () => {
				const newTarget = await getTargetData({ projectId });
				return writableStore.set({ isLoading: false, value: newTarget });
			}
		};
		this.targetBranchStores.set(projectId, refreshableStore);
		return refreshableStore;
	}
}

export async function listVirtualBranches(params: { projectId: string }): Promise<Branch[]> {
	return plainToInstance(Branch, await invoke<any[]>('list_virtual_branches', params));
}

export async function getRemoteBranchesData(params: { projectId: string }): Promise<BranchData[]> {
	return plainToInstance(BranchData, await invoke<any[]>('git_remote_branches_data', params));
}

export async function getTargetData(params: { projectId: string }): Promise<Target> {
	return plainToInstance(Target, invoke<any>('get_target_data', params));
}

async function branchesWithFileContent(projectId: string, sessionId: string, branches: Branch[]) {
	const filePaths = branches
		.map((branch) => branch.files)
		.flat()
		.map((file) => file.path);
	const fullFiles = await invoke<Record<string, string>>('list_session_files', {
		projectId: projectId,
		sessionId: sessionId,
		paths: filePaths
	});
	const branchesWithContnent = branches.map((branch) => {
		branch.files.map((file) => {
			file.content = fullFiles[file.path];
		});
		return branch;
	});
	return branchesWithContnent;
}

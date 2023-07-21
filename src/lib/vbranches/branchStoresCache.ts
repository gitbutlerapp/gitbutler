import { writable, type Loadable, Loaded } from 'svelte-loadable-store';
import type { Session, Delta } from '$lib/api';
import { Operation } from '$lib/api';
import type { Readable } from '@square/svelte-store';
import { deltas, git } from '$lib/api/ipc';
import { stores } from '$lib';
import { BaseBranch, Branch, BranchData } from './types';
import { plainToInstance } from 'class-transformer';
import { invoke } from '$lib/ipc';

export interface Refreshable {
	refresh(): Promise<void | object>;
}

export class BranchStoresCache {
	virtualBranchStores: Map<string, Refreshable & Readable<Loadable<Branch[]>>> = new Map();
	remoteBranchStores: Map<string, Refreshable & Readable<Loadable<BranchData[]>>> = new Map();
	targetBranchStores: Map<string, Refreshable & Readable<Loadable<BaseBranch | null>>> = new Map();

	getVirtualBranchStore(projectId: string) {
		const cachedStore = this.virtualBranchStores.get(projectId);
		if (cachedStore) {
			return cachedStore;
		}

		const writableStore = writable(listVirtualBranches({ projectId }), (set) => {
			return stores.sessions({ projectId }).subscribe((sessions) => {
				if (sessions.isLoading) return;
				if (Loaded.isError(sessions)) return;
				const lastSession = sessions.value.at(-1);
				if (!lastSession) return;
				if (lastSession.hash) return;

				// new current session detected. refresh branches + subscribe to delta updates.
				listVirtualBranches({ projectId }).then((newBranches) => {
					branchesWithFileContent(projectId, lastSession.id, newBranches).then((withContent) => {
						set(withContent);
					});
				});

				return deltas.subscribe({ projectId, sessionId: lastSession.id }, () => {
					// new delta detected. refresh branches.
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

	getBaseBranchStore(projectId: string) {
		const cachedStore = this.targetBranchStores.get(projectId);
		if (cachedStore) {
			return cachedStore;
		}
		const writableStore = writable(getBaseBranchData({ projectId }), (set) => {
			git.fetches.subscribe({ projectId }, () => {
				getBaseBranchData({ projectId }).then(set);
			});
		});
		const refreshableStore = {
			subscribe: writableStore.subscribe,
			refresh: async () => {
				const newBaseBranch = await getBaseBranchData({ projectId });
				return writableStore.set({ isLoading: false, value: newBaseBranch });
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

export async function getBaseBranchData(params: { projectId: string }): Promise<BaseBranch> {
	return plainToInstance(BaseBranch, invoke<any>('get_base_branch_data', params));
}

async function branchesWithFileContent(projectId: string, sessionId: string, branches: Branch[]) {
	const filePaths = branches
		.map((branch) => branch.files)
		.flat()
		.map((file) => file.path);
	const sessionFiles = await invoke<Record<string, string>>('list_session_files', {
		projectId: projectId,
		sessionId: sessionId,
		paths: filePaths
	});
	const sessionDeltas = await invoke<Record<string, Delta[]>>('list_deltas', {
		projectId: projectId,
		sessionId: sessionId,
		paths: filePaths
	});
	const branchesWithContnent = branches.map((branch) => {
		branch.files.map((file) => {
			const contentAtSessionStart = sessionFiles[file.path];
			const deltas = sessionDeltas[file.path];
			file.content = applyDeltas(contentAtSessionStart, deltas);
		});
		return branch;
	});
	return branchesWithContnent;
}

function applyDeltas(text: string, deltas: Delta[]) {
	if (!deltas) return text;
	const operations = deltas.flatMap((delta) => delta.operations);
	operations.forEach((operation) => {
		if (Operation.isInsert(operation)) {
			text =
				text.slice(0, operation.insert[0]) + operation.insert[1] + text.slice(operation.insert[0]);
		} else if (Operation.isDelete(operation)) {
			text =
				text.slice(0, operation.delete[0]) + text.slice(operation.delete[0] + operation.delete[1]);
		}
	});
	return text;
}

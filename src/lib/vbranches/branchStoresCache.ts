import { getSessionStore } from '$lib/stores/sessions';
import { asyncWritable } from '@square/svelte-store';
import * as fetches from '$lib/api/git/fetches';
import * as deltas from '$lib/api/ipc/deltas';
import { BaseBranch, Branch, BranchData, type Reloadable } from './types';
import { plainToInstance } from 'class-transformer';
import { invoke } from '$lib/ipc';
import { isDelete, isInsert } from '$lib/api/ipc/deltas';

export class BranchStoresCache {
	virtualBranchStores: Map<string, Reloadable<Branch[]>> = new Map();
	remoteBranchStores: Map<string, Reloadable<BranchData[]>> = new Map();
	targetBranchStores: Map<string, Reloadable<BaseBranch | undefined>> = new Map();

	getVirtualBranchStore(projectId: string) {
		const cachedStore = this.virtualBranchStores.get(projectId);
		if (cachedStore) {
			return cachedStore;
		}

		const writableStore = asyncWritable(
			[],
			async () => listVirtualBranches({ projectId }),
			async (newBranches) => newBranches,
			{ reloadable: true, trackState: true }
		) as Reloadable<Branch[]>;

		getSessionStore({ projectId }).subscribe((sessions) => {
			const lastSession = sessions?.at(-1);
			if (!lastSession) return;

			// new current session detected. refresh branches + subscribe to delta updates.
			listVirtualBranches({ projectId }).then((newBranches) => {
				branchesWithFileContent(projectId, lastSession.id, newBranches).then((withContent) => {
					writableStore.set(withContent);
				});
			});
			// TODO: We need to unsubscribe this somewhere!
			const unsubscribe2 = deltas.subscribe({ projectId, sessionId: lastSession.id }, () => {
				// new delta detected. refresh branches.
				listVirtualBranches({ projectId }).then((newBranches) => {
					branchesWithFileContent(projectId, lastSession.id, newBranches).then((withContent) => {
						writableStore.set(withContent);
					});
				});
			});
		});
		this.virtualBranchStores.set(projectId, writableStore);
		return writableStore;
	}

	getRemoteBranchStore(projectId: string) {
		const cachedStore = this.remoteBranchStores.get(projectId);
		if (cachedStore) {
			return cachedStore;
		}
		const writableStore = asyncWritable(
			[],
			async () => getRemoteBranchesData({ projectId }),
			async (newRemotes) => newRemotes,
			{ reloadable: true, trackState: true }
		) as Reloadable<BranchData[]>;
		// TODO: We need to unsubscribe this somewhere!
		const unsubscribe = fetches.subscribe({ projectId }, () => {
			getRemoteBranchesData({ projectId }).then((data) => writableStore.set(data));
		});
		this.remoteBranchStores.set(projectId, writableStore);
		return writableStore;
	}

	getBaseBranchStore(projectId: string) {
		const cachedStore = this.targetBranchStores.get(projectId);
		if (cachedStore) {
			return cachedStore;
		}
		const writableStore = asyncWritable(
			[],
			async () => await getBaseBranchData({ projectId }),
			async (newBaseBranch) => {
				return newBaseBranch;
			},
			{ reloadable: true, trackState: true }
		) as Reloadable<BaseBranch>;
		// TODO: We need to unsubscribe this somewhere!
		const unsubscribe = fetches.subscribe({ projectId }, () => {
			getBaseBranchData({ projectId }).then((data) => writableStore.set(data));
		});
		this.targetBranchStores.set(projectId, writableStore);
		return writableStore;
	}
}

export async function listVirtualBranches(params: { projectId: string }): Promise<Branch[]> {
	const result = plainToInstance(Branch, await invoke<any[]>('list_virtual_branches', params));
	result.forEach((branch) => {
		branch.files.sort((a) => (a.conflicted ? -1 : 0));
	});
	return result;
}

export async function getRemoteBranchesData(params: { projectId: string }): Promise<BranchData[]> {
	return plainToInstance(BranchData, await invoke<any[]>('git_remote_branches_data', params));
}

export async function getBaseBranchData(params: { projectId: string }): Promise<BaseBranch> {
	return (
		plainToInstance(BaseBranch, await invoke<any>('get_base_branch_data', params)) || undefined
	);
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
	const sessionDeltas = await invoke<Record<string, deltas.Delta[]>>('list_deltas', {
		projectId: projectId,
		sessionId: sessionId,
		paths: filePaths
	});
	const branchesWithContnent = branches.map((branch) => {
		branch.files.map((file) => {
			const contentAtSessionStart = sessionFiles[file.path];
			const ds = sessionDeltas[file.path];
			file.content = applyDeltas(contentAtSessionStart, ds);
		});
		return branch;
	});
	return branchesWithContnent;
}

function applyDeltas(text: string, ds: deltas.Delta[]) {
	if (!ds) return text;
	const operations = ds.flatMap((delta) => delta.operations);
	operations.forEach((operation) => {
		if (isInsert(operation)) {
			text =
				text.slice(0, operation.insert[0]) + operation.insert[1] + text.slice(operation.insert[0]);
		} else if (isDelete(operation)) {
			text =
				text.slice(0, operation.delete[0]) + text.slice(operation.delete[0] + operation.delete[1]);
		}
	});
	return text;
}

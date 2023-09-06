import { asyncWritable, type Readable } from '@square/svelte-store';
import { BaseBranch, Branch, BranchData, type WritableReloadable } from './types';
import { plainToInstance } from 'class-transformer';
import { invoke } from '$lib/ipc';
import { isDelete, isInsert, type Delta } from '$lib/api/ipc/deltas';
import type { Session } from '$lib/api/ipc/sessions';

export function getVirtualBranchStore(projectId: string, asyncStores: Readable<any>[]) {
	return asyncWritable(
		asyncStores,
		async () => await listVirtualBranches({ projectId }),
		async (newBranches) => newBranches,
		{ reloadable: true, trackState: true }
	) as WritableReloadable<Branch[] | undefined>;
}

export function getWithContentStore(
	projectId: string,
	sessionStore: Readable<Session[]>,
	vbranchStore: Readable<Branch[] | undefined>
) {
	return asyncWritable(
		[vbranchStore, sessionStore],
		async ([branches, sessions]) => {
			const lastSession = sessions.at(-1);
			return lastSession ? await withFileContent(projectId, lastSession.id, branches) : [];
		},
		async (newBranches) => newBranches,
		{ reloadable: true, trackState: true }
	) as WritableReloadable<Branch[] | undefined>;
}

export function getRemoteBranchStore(projectId: string, asyncStores: Readable<any>[]) {
	return asyncWritable(
		asyncStores,
		async () => getRemoteBranchesData({ projectId }),
		async (newRemotes) => newRemotes,
		{ reloadable: true, trackState: true }
	) as WritableReloadable<BranchData[] | undefined>;
}

export function getBaseBranchStore(projectId: string, asyncStores: Readable<any>[]) {
	return asyncWritable(
		asyncStores,
		async () => getBaseBranchData({ projectId }),
		async (newBaseBranch) => newBaseBranch,
		{ reloadable: true, trackState: true }
	) as WritableReloadable<BaseBranch | undefined>;
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
	const baseBranch = plainToInstance(BaseBranch, await invoke<any>('get_base_branch_data', params));
	if (baseBranch) {
		// The rust code performs a fetch when get_base_branch_data is invoked
		baseBranch.fetchedAt = new Date();
	}
	return baseBranch || undefined;
}

export async function withFileContent(
	projectId: string,
	sessionId: string,
	branches: Branch[] | undefined
) {
	if (!branches) {
		return [];
	}
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
			const ds = sessionDeltas[file.path];
			file.content = applyDeltas(contentAtSessionStart, ds);
		});
		return branch;
	});
	return branchesWithContnent;
}

function applyDeltas(text: string, ds: Delta[]) {
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

import { invoke } from '$lib/ipc';
import { plainToInstance } from 'class-transformer';
import { Branch, BranchData, Target } from './types';

export async function listVirtualBranches(params: { projectId: string }): Promise<Branch[]> {
	const result = await invoke<any[]>('list_virtual_branches', params);
	return sortBranches(plainToInstance(Branch, result));
}

export async function create(params: {
	projectId: string;
	branch: {
		name?: string;
		ownership?: string;
		order?: number;
	};
}) {
	return await invoke<void>('create_virtual_branch', params);
}

export async function commit(params: { projectId: string; branch: string; message: string }) {
	return await invoke<void>('commit_virtual_branch', params);
}

export async function update(params: {
	projectId: string;
	branch: {
		id: string;
		order?: number;
		ownership?: string;
		name?: string;
	};
}) {
	return await invoke<void>('update_virtual_branch', params);
}

async function del(params: { projectId: string; branchId: string }) {
	return await invoke<void>('delete_virtual_branch', params);
}
export { del as delete };

export async function push(params: { projectId: string; branchId: string }) {
	return await invoke<void>('push_virtual_branch', params);
}

export async function apply(params: { projectId: string; branch: string }) {
	return await invoke<void>('apply_branch', params);
}

export async function unapply(params: { projectId: string; branch: string }) {
	return await invoke<void>('unapply_branch', params);
}

export async function getRemoteBranchesData(params: { projectId: string }) {
	return invoke<Array<BranchData>>('git_remote_branches_data', params);
}

export async function getTargetData(params: { projectId: string }) {
	return invoke<Target | null>('get_target_data', params);
}

export async function setTarget(params: { projectId: string; branch: string }) {
	return await invoke<Target>('set_target_branch', params);
}

export async function updateBranchTarget(params: { projectId: string }) {
	return invoke<object>('update_branch_target', params);
}

export async function createvBranchFromBranch(params: { projectId: string; branch: string }) {
	return invoke<string>('create_virtual_branch_from_branch', params);
}

export async function fetchFromTarget(params: { projectId: string }) {
	return invoke<void>('fetch_from_target', params);
}

function sortBranches(branches: Branch[]): Branch[] {
	branches.sort((a, b) => a.order - b.order);
	return branches;
}

import 'reflect-metadata';
import { invoke } from '$lib/ipc';
import { plainToInstance, Type, Transform } from 'class-transformer';

export class Hunk {
	id!: string;
	name!: string;
	diff!: string;
	@Transform((obj) => {
		return new Date(obj.value);
	})
	modifiedAt!: Date;
	filePath!: string;
}

export class File {
	id!: string;
	path!: string;
	@Type(() => Hunk)
	hunks!: Hunk[];
	expanded?: boolean;
}

export class Branch {
	id!: string;
	name!: string;
	active!: boolean;
	@Type(() => File)
	files!: File[];
	commits!: Commit[];
	description!: string;
	mergeable!: boolean;
	mergeConflicts!: string[];
	order!: number;
	upstream!: string;
}

export class Commit {
	id!: string;
	authorEmail!: string;
	authorName!: string;
	description!: string;
	@Transform((obj) => new Date(obj.value))
	createdAt!: Date;
	isRemote!: boolean;
}

export async function list(params: { projectId: string }): Promise<Branch[]> {
	const result = await invoke<any[]>('list_virtual_branches', params);
	return plainToInstance(Branch, result);
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

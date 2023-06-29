import { invoke } from '$lib/ipc';
import { Branch } from './types';
import { stores } from '$lib';
import { writable, type Loadable, Value } from 'svelte-loadable-store';
import { plainToInstance } from 'class-transformer';
import type { Writable, Readable } from '@square/svelte-store';
import { error } from '$lib/toasts';

const cache: Map<string, VirtualBranchOperations & Readable<Loadable<Branch[]>>> = new Map();

export interface VirtualBranchOperations {
	setTarget(branch: string): Promise<object>;
	createBranch(name: string, path: string): Promise<void | object>;
	commitBranch(branch: string, message: string): Promise<void | object>;
	updateBranchTarget(): Promise<void | object>;
	updateBranchName(branchId: string, name: string): Promise<void | object>;
	applyBranch(branchId: string): Promise<void | object>;
	unapplyBranch(branchId: string): Promise<void | object>;
	moveFiles(branchId: string, paths: Array<string>): Promise<void | object>;
}

export function getVirtualBranches(
	projectId: string
): VirtualBranchOperations & Readable<Loadable<Branch[]>> {
	const cachedStore = cache.get(projectId);
	if (cachedStore) {
		return cachedStore;
	}
	const writeable = createWriteable(projectId);
	const store: VirtualBranchOperations & Readable<Loadable<Branch[]>> = {
		subscribe: writeable.subscribe,
		setTarget: (branch) => setTarget(projectId, branch),
		createBranch: (name, path) => createBranch(writeable, projectId, name, path),
		commitBranch: (branch, message) => commitBranch(writeable, projectId, branch, message),
		updateBranchTarget: () => updateBranchTarget(writeable, projectId),
		updateBranchName: (branchId, name) => updateBranchName(writeable, projectId, branchId, name),
		applyBranch: (branchId) => applyBranch(writeable, projectId, branchId),
		unapplyBranch: (branchId) => unapplyBranch(writeable, projectId, branchId),
		moveFiles: (branchId, paths) => moveFiles(writeable, projectId, branchId, paths)
	};
	cache.set(projectId, store);
	return store;
}

function createWriteable(projectId: string) {
	// Subscribe to sessions,  grab the last one and subscribe to deltas on it.
	// When a delta comes in, refresh the list of virtual branches.
	return writable(list(projectId), (set) => {
		const sessionsUnsubscribe = stores.sessions({ projectId }).subscribe((sessions) => {
			if (sessions.isLoading) return;
			if (Value.isError(sessions.value)) return;
			const lastSession = sessions.value.at(0);
			if (!lastSession) return;
			const deltasUnsubscribe = stores
				.deltas({ projectId, sessionId: lastSession.id })
				.subscribe(() => {
					list(projectId).then((newBranches) => {
						set(sort(plainToInstance(Branch, newBranches)));
					});
					return () => deltasUnsubscribe();
				});
			return () => sessionsUnsubscribe();
		});
	});
}

function refresh(projectId: string, store: Writable<Loadable<Branch[]>>) {
	list(projectId).then((newBranches) => store.set({ isLoading: false, value: newBranches }));
}

function sort(branches: Branch[]): Branch[] {
	for (const branch of branches) {
		for (const file of branch.files) {
			file.hunks.sort((a, b) => b.modifiedAt.getTime() - a.modifiedAt.getTime());
		}
	}
	return branches;
}

async function list(projectId: string): Promise<Branch[]> {
	return invoke<Array<Branch>>('list_virtual_branches', { projectId }).then((result) =>
		sort(plainToInstance(Branch, result))
	);
}

function setTarget(projectId: string, branch: string) {
	return invoke<object>('set_target_branch', {
		projectId: projectId,
		branch: branch
	});
}

function createBranch(
	writable: Writable<Loadable<Branch[]>>,
	projectId: string,
	name: string,
	path: string
) {
	return invoke<object>('create_virtual_branch', {
		projectId: projectId,
		name: name,
		path: path
	}).then(() => refresh(projectId, writable));
}

function commitBranch(
	writable: Writable<Loadable<Branch[]>>,
	projectId: string,
	branch: string,
	message: string
) {
	return invoke<object>('commit_virtual_branch', {
		projectId: projectId,
		branch: branch,
		message: message
	})
		.then((res) => {
			console.log(res);
			refresh(projectId, writable);
		})
		.catch((err) => {
			console.log(err);
			error('Failed to commit files.');
		});
}

function updateBranchTarget(writable: Writable<Loadable<Branch[]>>, projectId: string) {
	return invoke<object>('update_branch_target', { projectId: projectId })
		.then((res) => {
			console.log(res);
			refresh(projectId, writable);
		})
		.catch((err) => {
			console.log(err);
			error('Unable to update target!');
		});
}

function applyBranch(writable: Writable<Loadable<Branch[]>>, projectId: string, branchId: string) {
	return invoke<object>('apply_branch', {
		projectId: projectId,
		branch: branchId
	})
		.then((res) => {
			console.log(res);
			refresh(projectId, writable);
		})
		.catch((err) => {
			console.log(err);
			error('Unable to apply branch!');
		});
}

function unapplyBranch(
	writable: Writable<Loadable<Branch[]>>,
	projectId: string,
	branchId: string
) {
	return invoke<object>('unapply_branch', {
		projectId: projectId,
		branch: branchId
	})
		.then((res) => {
			console.log(res);
			refresh(projectId, writable);
		})
		.catch((err) => {
			console.log(err);
			error('Unable to unapply branch!');
		});
}

function updateBranchName(
	writable: Writable<Loadable<Branch[]>>,
	projectId: string,
	branchId: string,
	name: string
) {
	return invoke<object>('update_virtual_branch', {
		projectId: projectId,
		branch: { id: branchId, name: name }
	})
		.then((res) => {
			console.log(res);
			refresh(projectId, writable);
		})
		.catch((err) => {
			console.log(err);
			error('Unable to update branch!');
		});
}

function moveFiles(
	writable: Writable<Loadable<Branch[]>>,
	projectId: string,

	branchId: string,
	paths: Array<string>
) {
	return invoke<object>('move_virtual_branch_files', {
		projectId: projectId,
		branch: branchId,
		paths: paths
	})
		.then((res) => {
			console.log(res);
			refresh(projectId, writable);
		})
		.catch((err) => {
			console.log(err);
			error('Unable to move files!');
		});
}

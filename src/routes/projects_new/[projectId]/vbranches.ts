import { invoke } from '$lib/ipc';
import { Branch } from './types';
import { stores } from '$lib';
import { writable, type Loadable, Value } from 'svelte-loadable-store';
import { plainToInstance } from 'class-transformer';
import type { Subscriber, Unsubscriber, Writable } from '@square/svelte-store';
import { error } from '$lib/toasts';

const cache: Map<string, VirtualBranchStore> = new Map();

export interface VirtualBranchStore {
	subscribe: (branches: Subscriber<Loadable<Branch[]>>) => Unsubscriber;
	refresh: () => void;
	setTarget: (branch: string) => Promise<object>;
	createBranch: (name: string, path: string) => Promise<void | object>;
	commitBranch: (branch: string, message: string) => Promise<void | object>;
	updateBranchTarget: () => Promise<void | object>;
	updateBranchName: (branchId: string, name: string) => Promise<void | object>;
	moveFiles: (branchId: string, paths: Array<string>) => Promise<void | object>;
}

export function getStore(projectId: string): VirtualBranchStore {
	const cachedStore = cache.get(projectId);
	if (cachedStore) {
		return cachedStore;
	}

	const writeable = createWriteable(projectId);
	const store: VirtualBranchStore = {
		subscribe: writeable.subscribe,
		refresh: () => refresh(projectId, writeable),
		setTarget(branch) {
			return invoke<object>('set_target_branch', {
				projectId: projectId,
				branch: branch
			});
		},
		createBranch(name, path) {
			return invoke<object>('create_virtual_branch', {
				projectId: projectId,
				name: name,
				path: path
			}).then(() => refresh(projectId, writeable));
		},
		commitBranch(branch, message) {
			return invoke<object>('commit_virtual_branch', {
				projectId: projectId,
				branch: branch,
				message: message
			})
				.then((res) => {
					console.log(res);
					refresh(projectId, writeable);
				})
				.catch((err) => {
					console.log(err);
					error('Failed to commit files.');
				});
		},
		updateBranchTarget() {
			return invoke<object>('update_branch_target', { projectId: projectId })
				.then((res) => {
					// TODO
					// We need to refetch target data here
					console.log(res);
				})
				.catch((err) => {
					console.log(err);
					error('Unable to update target!');
				});
		},
		updateBranchName(branchId, name) {
			return invoke<object>('update_virtual_branch', {
				projectId: projectId,
				branch: { id: branchId, name: name }
			})
				.then((res) => {
					console.log(res);
					refresh(projectId, writeable);
				})
				.catch((err) => {
					console.log(err);
					error('Unable to update branch!');
				});
		},
		moveFiles(branchId, paths) {
			return invoke<object>('move_virtual_branch_files', {
				projectId: projectId,
				branch: branchId,
				paths: paths
			})
				.then((res) => {
					console.log(res);
					refresh(projectId, writeable);
				})
				.catch((err) => {
					console.log(err);
					error('Unable to move files!');
				});
		}
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

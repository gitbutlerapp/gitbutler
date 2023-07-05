import { invoke } from '$lib/ipc';
import type { Target } from './types';
import { stores, toasts } from '$lib';
import { api } from '$lib';
import { writable, type Loadable, Value } from 'svelte-loadable-store';
import type { Writable, Readable } from '@square/svelte-store';

const cache: Map<string, VirtualBranchOperations & Readable<Loadable<api.vbranches.Branch[]>>> =
	new Map();

export interface VirtualBranchOperations {
	setTarget(branch: string): Promise<void | Target>;
	createBranch(name: string, path: string): Promise<void | object>;
	commitBranch(branch: string, message: string): Promise<void | object>;
	updateBranchName(branchId: string, name: string): Promise<void | object>;
	updateBranchOrder(branchId: string, order: number): Promise<void | object>;
	applyBranch(branchId: string): Promise<void | object>;
	unapplyBranch(branchId: string): Promise<void | object>;
	updateBranchOwnership(branchId: string, ownership: string): Promise<void | object>;
	pushBranch(branchId: string): Promise<void | object>;
	deleteBranch(branchId: string): Promise<void | object>;
	refresh(): Promise<void | object>;
}

export function getVirtualBranches(
	projectId: string
): VirtualBranchOperations & Readable<Loadable<api.vbranches.Branch[]>> {
	const cachedStore = cache.get(projectId);
	if (cachedStore) {
		return cachedStore;
	}
	const writeable = createWriteable(projectId);
	const store: VirtualBranchOperations & Readable<Loadable<api.vbranches.Branch[]>> = {
		subscribe: writeable.subscribe,
		setTarget: (branch) =>
			setTarget(projectId, branch)
				.then((t) => {
					refresh(projectId, writeable);
					return t;
				})
				.catch((err) => {
					console.error(err);
					toasts.error('Failed to set target');
				}),
		createBranch: (name, path) =>
			api.vbranches
				.create({ projectId, name, ownership: path })
				.then(() => refresh(projectId, writeable))
				.catch((err) => {
					console.error(err);
					toasts.error('Failed to create branch');
				}),
		commitBranch: (branch, message) =>
			api.vbranches
				.commit({ projectId, branch, message })
				.then(() => refresh(projectId, writeable))
				.catch((err) => {
					console.error(err);
					toasts.error('Failed to commit branch');
				}),
		updateBranchOrder: (branchId, order) =>
			api.vbranches
				.update({ projectId, branch: { id: branchId, order } })
				.then(() => refresh(projectId, writeable))
				.catch((err) => {
					console.error(err);
					toasts.error('Failed to update branch order');
				}),
		updateBranchName: (branchId, name) =>
			api.vbranches
				.update({ projectId, branch: { id: branchId, name } })
				.then(() => refresh(projectId, writeable))
				.catch((err) => {
					console.error(err);
					toasts.error('Failed to update branch name');
				}),
		applyBranch: (branchId) =>
			api.vbranches
				.apply({ projectId, branch: branchId })
				.then(() => refresh(projectId, writeable))
				.catch((err) => {
					console.error(err);
					toasts.error('Failed to apply branch');
				}),
		unapplyBranch: (branchId) =>
			api.vbranches
				.unapply({ projectId, branch: branchId })
				.then(() => refresh(projectId, writeable))
				.catch((err) => {
					console.error(err);
					toasts.error('Failed to unapply branch');
				}),
		updateBranchOwnership: (branchId, ownership) =>
			api.vbranches
				.update({ projectId, branch: { id: branchId, ownership } })
				.then(() => refresh(projectId, writeable))
				.catch((err) => {
					console.error(err);
					toasts.error('Failed to update branch ownership');
				}),
		pushBranch: (branchId) =>
			api.vbranches
				.push({ projectId, branchId })
				.then(() => refresh(projectId, writeable))
				.catch((err) => {
					console.error(err);
					toasts.error('Failed to push branch');
				}),
		deleteBranch: (branchId) =>
			api.vbranches
				.delete({ projectId, branchId })
				.then(() => refresh(projectId, writeable))
				.catch((err) => {
					console.error(err);
					toasts.error('Failed to delete branch');
				}),
		refresh: () => refresh(projectId, writeable)
	};
	cache.set(projectId, store);
	return store;
}

function createWriteable(projectId: string) {
	// Subscribe to sessions,  grab the last one and subscribe to deltas on it.
	// When a delta comes in, refresh the list of virtual branches.
	return writable(api.vbranches.list({ projectId }), (set) => {
		stores.sessions({ projectId }).subscribe((sessions) => {
			if (sessions.isLoading) return;
			if (Value.isError(sessions.value)) return;
			const lastSession = sessions.value.at(-1);
			if (!lastSession) return;
			return stores.deltas({ projectId, sessionId: lastSession.id }).subscribe(() => {
				api.vbranches.list({ projectId }).then(set);
			});
		});
	});
}

async function refresh(projectId: string, store: Writable<Loadable<api.vbranches.Branch[]>>) {
	return await api.vbranches
		.list({ projectId })
		.then((newBranches) => store.set({ isLoading: false, value: newBranches }));
}

async function setTarget(projectId: string, branch: string) {
	return await invoke<Target>('set_target_branch', {
		projectId: projectId,
		branch: branch
	});
}

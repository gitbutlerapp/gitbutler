import type { Refreshable } from './branchStores';
import type { Readable } from '@square/svelte-store';
import type { Loadable } from 'svelte-loadable-store';
import type { Branch, BranchData, Target } from './types';
import { toasts } from '$lib';
import * as ipc from './ipc';

export class BranchController {
	constructor(
		readonly projectId: string,
		readonly virtualBranchStore: Refreshable & Readable<Loadable<Branch[]>>,
		readonly remoteBranchStore: Refreshable & Readable<Loadable<BranchData[]>>,
		readonly targetBranchStore: Refreshable & Readable<Loadable<Target>>
	) {}

	setTarget(branch: string) {
		const projectId = this.projectId;
		return ipc
			.setTarget({ projectId, branch })
			.then(this.virtualBranchStore.refresh)
			.then(this.remoteBranchStore.refresh)
			.catch((err) => {
				console.log(err);
				toasts.error('Failed to set target');
			});
	}

	createBranch(branch: { name?: string; ownership?: string; order?: number }) {
		const projectId = this.projectId;
		return ipc
			.create({ projectId, branch })
			.then(this.virtualBranchStore.refresh)
			.catch((err) => {
				console.error(err);
				toasts.error('Failed to create branch');
			});
	}

	commitBranch(branch: string, message: string) {
		const projectId = this.projectId;
		return ipc
			.commit({ projectId, branch, message })
			.then(this.virtualBranchStore.refresh)
			.catch((err) => {
				console.error(err);
				toasts.error('Failed to commit branch');
			});
	}

	updateBranchName(branchId: string, name: string) {
		const projectId = this.projectId;
		return ipc
			.update({ projectId, branch: { id: branchId, name } })
			.then(this.virtualBranchStore.refresh)
			.catch((err) => {
				console.error(err);
				toasts.error('Failed to update branch name');
			});
	}

	updateBranchOrder(branchId: string, order: number) {
		const projectId = this.projectId;
		return ipc
			.update({ projectId, branch: { id: branchId, order } })
			.then(this.virtualBranchStore.refresh)
			.catch((err) => {
				console.error(err);
				toasts.error('Failed to update branch order');
			});
	}

	applyBranch(branchId: string) {
		const projectId = this.projectId;
		return ipc
			.apply({ projectId, branch: branchId })
			.then(this.virtualBranchStore.refresh)
			.catch((err) => {
				console.error(err);
				toasts.error('Failed to apply branch');
			});
	}

	unapplyBranch(branchId: string) {
		const projectId = this.projectId;
		return ipc
			.unapply({ projectId, branch: branchId })
			.then(this.virtualBranchStore.refresh)
			.catch((err) => {
				console.error(err);
				toasts.error('Failed to unapply branch');
			});
	}

	updateBranchOwnership(branchId: string, ownership: string) {
		const projectId = this.projectId;
		return ipc
			.update({ projectId, branch: { id: branchId, ownership } })
			.then(this.virtualBranchStore.refresh)
			.catch((err) => {
				console.error(err);
				toasts.error('Failed to update branch ownership');
			});
	}

	pushBranch(branchId: string) {
		const projectId = this.projectId;
		return ipc
			.push({ projectId, branchId })
			.then(this.virtualBranchStore.refresh)
			.catch((err) => {
				console.error(err);
				toasts.error('Failed to push branch');
			});
	}

	deleteBranch(branchId: string) {
		const projectId = this.projectId;
		return ipc
			.delete({ projectId, branchId })
			.then(this.virtualBranchStore.refresh)
			.then(this.remoteBranchStore.refresh)
			.catch((err) => {
				console.error(err);
				toasts.error('Failed to delete branch');
			});
	}

	// remote

	updateBranchTarget() {
		const projectId = this.projectId;
		return ipc
			.updateBranchTarget({ projectId })
			.then(this.remoteBranchStore.refresh)
			.then(this.virtualBranchStore.refresh)
			.catch((err) => {
				console.error(err);
				toasts.error('Failed to update target');
			});
	}

	createvBranchFromBranch(branch: string): Promise<void | string> {
		const projectId = this.projectId;
		const result = ipc.createvBranchFromBranch({ projectId, branch });
		result.then(this.remoteBranchStore.refresh);
		result.then(this.virtualBranchStore.refresh);
		result.catch((err) => {
			console.error(err);
			toasts.error('Failed to create virtual branch from branch');
		});
		// is this return right?
		return result;
	}

	fetchFromTarget() {
		const projectId = this.projectId;
		return ipc
			.fetchFromTarget({ projectId })
			.then(this.remoteBranchStore.refresh)
			.then(this.targetBranchStore.refresh)
			.catch((err) => {
				console.error(err);
				toasts.error('Failed to fetch from target');
			});
	}
}

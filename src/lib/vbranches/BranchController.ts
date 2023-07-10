import type { Refreshable } from './BranchStoresCache';
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

	async setTarget(branch: string) {
		try {
			await ipc.setTarget({ projectId: this.projectId, branch });
			await this.virtualBranchStore.refresh();
			await this.remoteBranchStore.refresh();
		} catch (err) {
			console.log(err);
			toasts.error('Failed to set target');
		}
	}

	async createBranch(branch: { name?: string; ownership?: string; order?: number }) {
		try {
			await ipc.create({ projectId: this.projectId, branch });
			await this.virtualBranchStore.refresh();
		} catch (err) {
			console.error(err);
			toasts.error('Failed to create branch');
		}
	}

	async commitBranch(branch: string, message: string) {
		try {
			await ipc.commit({ projectId: this.projectId, branch, message });
			await this.virtualBranchStore.refresh();
		} catch (err) {
			console.error(err);
			toasts.error('Failed to commit branch');
		}
	}

	async updateBranchName(branchId: string, name: string) {
		try {
			await ipc.update({ projectId: this.projectId, branch: { id: branchId, name } });
			await this.virtualBranchStore.refresh();
		} catch (err) {
			console.error(err);
			toasts.error('Failed to update branch name');
		}
	}

	async updateBranchOrder(branchId: string, order: number) {
		try {
			await ipc.update({ projectId: this.projectId, branch: { id: branchId, order } });
			await this.virtualBranchStore.refresh();
		} catch (err) {
			console.error(err);
			toasts.error('Failed to update branch order');
		}
	}

	async applyBranch(branchId: string) {
		try {
			await ipc.apply({ projectId: this.projectId, branch: branchId });
			await this.virtualBranchStore.refresh();
		} catch (err) {
			console.error(err);
			toasts.error('Failed to apply branch');
		}
	}

	async unapplyBranch(branchId: string) {
		try {
			await ipc.unapply({ projectId: this.projectId, branch: branchId });
			await this.virtualBranchStore.refresh();
		} catch (err) {
			console.error(err);
			toasts.error('Failed to unapply branch');
		}
	}

	async updateBranchOwnership(branchId: string, ownership: string) {
		try {
			await ipc.update({ projectId: this.projectId, branch: { id: branchId, ownership } });
			await this.virtualBranchStore.refresh();
		} catch (err) {
			console.error(err);
			toasts.error('Failed to update branch ownership');
		}
	}

	async pushBranch(branchId: string) {
		try {
			await ipc.push({ projectId: this.projectId, branchId });
			await this.virtualBranchStore.refresh();
		} catch (err) {
			console.error(err);
			toasts.error('Failed to push branch');
		}
	}

	async deleteBranch(branchId: string) {
		try {
			await ipc.delete({ projectId: this.projectId, branchId });
			await this.virtualBranchStore.refresh();
			await this.remoteBranchStore.refresh();
		} catch (err) {
			console.error(err);
			toasts.error('Failed to delete branch');
		}
	}

	async updateBranchTarget() {
		try {
			await ipc.updateBranchTarget({ projectId: this.projectId });
			await this.remoteBranchStore.refresh();
			await this.virtualBranchStore.refresh();
		} catch (err) {
			console.error(err);
			toasts.error('Failed to update target');
		}
	}

	async createvBranchFromBranch(branch: string) {
		try {
			await ipc.createvBranchFromBranch({ projectId: this.projectId, branch });
			await this.remoteBranchStore.refresh();
			await this.targetBranchStore.refresh();
		} catch (err) {
			console.error(err);
			toasts.error('Failed to create virtual branch from branch');
		}
	}

	async fetchFromTarget() {
		try {
			await ipc.fetchFromTarget({ projectId: this.projectId });
			await this.remoteBranchStore.refresh();
			await this.targetBranchStore.refresh();
		} catch (err) {
			console.error(err);
			toasts.error('Failed to fetch from target');
		}
	}
}

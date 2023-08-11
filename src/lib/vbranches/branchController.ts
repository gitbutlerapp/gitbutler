import type { Branch, BranchData, BaseBranch, WritableReloadable } from './types';
import * as toasts from '$lib/toasts';
import { invoke } from '$lib/ipc';

export class BranchController {
	constructor(
		readonly projectId: string,
		readonly virtualBranchStore: WritableReloadable<Branch[] | undefined>,
		readonly remoteBranchStore: WritableReloadable<BranchData[] | undefined>,
		readonly targetBranchStore: WritableReloadable<BaseBranch | undefined>
	) {}

	async setTarget(branch: string) {
		try {
			await invoke<BaseBranch>('set_base_branch', { projectId: this.projectId, branch });
			await Promise.all([
				this.virtualBranchStore.reload(),
				this.remoteBranchStore.reload(),
				this.targetBranchStore.reload()
			]);
		} catch (err) {
			toasts.error('Failed to set target');
		}
	}

	async createBranch(branch: { name?: string; ownership?: string; order?: number }) {
		try {
			await invoke<void>('create_virtual_branch', { projectId: this.projectId, branch });
			await this.virtualBranchStore.reload();
		} catch (err) {
			toasts.error('Failed to create branch');
		}
	}

	async commitBranch(branch: string, message: string) {
		try {
			await invoke<void>('commit_virtual_branch', { projectId: this.projectId, branch, message });
			await this.virtualBranchStore.reload();
		} catch (err) {
			toasts.error('Failed to commit branch');
		}
	}

	async updateBranchName(branchId: string, name: string) {
		try {
			await invoke<void>('update_virtual_branch', {
				projectId: this.projectId,
				branch: { id: branchId, name }
			});
			await this.virtualBranchStore.reload();
		} catch (err) {
			toasts.error('Failed to update branch name');
		}
	}

	async updateBranchNotes(branchId: string, notes: string) {
		try {
			await invoke<void>('update_virtual_branch', {
				projectId: this.projectId,
				branch: { id: branchId, notes }
			});
			await this.virtualBranchStore.reload();
		} catch (err) {
			toasts.error('Failed to update branch notes');
		}
	}

	async updateBranchOrder(branchId: string, order: number) {
		try {
			await invoke<void>('update_virtual_branch', {
				projectId: this.projectId,
				branch: { id: branchId, order }
			});
			await this.virtualBranchStore.reload();
		} catch (err) {
			toasts.error('Failed to update branch order');
		}
	}

	async applyBranch(branchId: string) {
		try {
			await invoke<void>('apply_branch', { projectId: this.projectId, branch: branchId });
			await this.virtualBranchStore.reload();
		} catch (err) {
			toasts.error('Failed to apply branch');
		}
	}

	async unapplyBranch(branchId: string) {
		try {
			await invoke<void>('unapply_branch', { projectId: this.projectId, branch: branchId });
			await this.virtualBranchStore.reload();
		} catch (err) {
			toasts.error('Failed to unapply branch');
		}
	}

	async updateBranchOwnership(branchId: string, ownership: string) {
		try {
			await invoke<void>('update_virtual_branch', {
				projectId: this.projectId,
				branch: { id: branchId, ownership }
			});
			await this.virtualBranchStore.reload();
		} catch (err) {
			toasts.error('Failed to update branch ownership');
		}
	}

	async pushBranch(branchId: string) {
		try {
			await invoke<void>('push_virtual_branch', { projectId: this.projectId, branchId });
			await this.virtualBranchStore.reload();
		} catch (err: any) {
			if (err.code === 'errors.git.authentication') {
				toasts.error('Failed to authenticate. Did you setup GitButler ssh keys?');
			} else {
				toasts.error(`Failed to push branch: ${err.message}`);
			}
		}
	}

	async deleteBranch(branchId: string) {
		try {
			await invoke<void>('delete_virtual_branch', { projectId: this.projectId, branchId });
			await this.virtualBranchStore.reload();
			await this.remoteBranchStore.reload();
		} catch (err) {
			toasts.error('Failed to delete branch');
		}
	}

	async updateBaseBranch() {
		try {
			await invoke<object>('update_base_branch', { projectId: this.projectId });
			await Promise.all([
				this.remoteBranchStore.reload(),
				this.virtualBranchStore.reload(),
				this.targetBranchStore.reload()
			]);
		} catch (err) {
			toasts.error('Failed to update target');
		}
	}

	async createvBranchFromBranch(branch: string) {
		try {
			await invoke<string>('create_virtual_branch_from_branch', {
				projectId: this.projectId,
				branch
			});
			await Promise.all([
				await this.remoteBranchStore.reload(),
				await this.targetBranchStore.reload()
			]);
		} catch (err) {
			toasts.error('Failed to create virtual branch from branch');
		}
	}

	async fetchFromTarget() {
		try {
			await invoke<void>('fetch_from_target', { projectId: this.projectId });
			await Promise.all([
				await this.remoteBranchStore.reload(),
				await this.targetBranchStore.reload()
			]);
		} catch (err: any) {
			if (err.code === 'errors.git.authentication') {
				toasts.error('Failed to authenticate. Did you setup GitButler ssh keys?');
			} else {
				toasts.error(`Failed to push branch: ${err.message}`);
			}
		}
	}

	async markResolved(projectId: string, path: string) {
		try {
			await invoke<void>('mark_resolved', { projectId, path });
			await this.virtualBranchStore.reload();
		} catch (err) {
			toasts.error(`Failed to mark file resolved`);
		}
	}
}

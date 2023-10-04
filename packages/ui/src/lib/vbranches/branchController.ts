import type {
	Branch,
	RemoteBranch,
	BaseBranch,
	CustomStore,
	Hunk,
	VirtualBranchStore
} from './types';
import * as toasts from '$lib/toasts';
import { invoke } from '$lib/ipc';

export class BranchController {
	constructor(
		readonly projectId: string,
		readonly virtualBranchStore: VirtualBranchStore<Branch>,
		readonly remoteBranchStore: CustomStore<RemoteBranch[] | undefined>,
		readonly targetBranchStore: CustomStore<BaseBranch | undefined>
	) {}

	async setTarget(branch: string) {
		try {
			await invoke<BaseBranch>('set_base_branch', { projectId: this.projectId, branch });
			await this.targetBranchStore.reload();
		} catch (err) {
			toasts.error('Failed to set base branch');
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

	async commitBranch(params: { branch: string; message: string; ownership?: string }) {
		try {
			await invoke<void>('commit_virtual_branch', { projectId: this.projectId, ...params });
			await this.virtualBranchStore.reload();
		} catch (err) {
			toasts.error('Failed to commit branch');
		}
	}

	async mergeUpstream(branch: string) {
		try {
			await invoke<void>('merge_virtual_branch_upstream', { projectId: this.projectId, branch });
			await this.virtualBranchStore.reload();
		} catch (err) {
			toasts.error('Failed to merge upstream branch');
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
			this.virtualBranchStore.updateById(branchId, (branch) => (branch.notes = notes));
			await invoke<void>('update_virtual_branch', {
				projectId: this.projectId,
				branch: { id: branchId, notes }
			});
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
			this.virtualBranchStore.updateById(branchId, (branch) => (branch.active = true));
			await invoke<void>('apply_branch', { projectId: this.projectId, branch: branchId });
		} catch (err) {
			toasts.error('Failed to apply branch');
		} finally {
			await this.virtualBranchStore.reload();
		}
	}

	async unapplyHunk(hunk: Hunk) {
		const ownership = `${hunk.filePath}:${hunk.id}`;
		try {
			await invoke<void>('unapply_ownership', { projectId: this.projectId, ownership });
			await this.virtualBranchStore.reload();
		} catch (err) {
			toasts.error('Failed to unapply hunk');
		}
	}

	async unapplyBranch(branchId: string) {
		try {
			this.virtualBranchStore.updateById(branchId, (branch) => (branch.active = false));
			await invoke<void>('unapply_branch', { projectId: this.projectId, branch: branchId });
		} catch (err) {
			toasts.error('Failed to unapply branch');
		} finally {
			await this.virtualBranchStore.reload();
		}
	}

	async updateBranchOwnership(branchId: string, ownership: string) {
		try {
			await invoke<void>('update_virtual_branch', {
				projectId: this.projectId,
				branch: { id: branchId, ownership }
			});
		} catch (err) {
			toasts.error('Failed to update branch ownership');
		}
		await this.virtualBranchStore.reload();
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
			await this.virtualBranchStore.update((branches) => branches?.filter((b) => b.id != branchId));
			await invoke<void>('delete_virtual_branch', { projectId: this.projectId, branchId });
		} catch (err) {
			toasts.error('Failed to delete branch');
		} finally {
			await this.virtualBranchStore.reload();
			await this.remoteBranchStore.reload();
		}
	}

	async updateBaseBranch() {
		try {
			await invoke<object>('update_base_branch', { projectId: this.projectId });
			await this.targetBranchStore.reload();
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
				await this.virtualBranchStore.reload(),
				await this.targetBranchStore.reload()
			]);
		} catch (err) {
			toasts.error('Failed to create virtual branch from branch');
		}
	}

	async fetchFromTarget() {
		try {
			await invoke<void>('fetch_from_target', { projectId: this.projectId });
			await this.targetBranchStore.reload();
		} catch (err: any) {
			if (err.code === 'errors.git.authentication') {
				toasts.error('Failed to authenticate. Did you setup GitButler ssh keys?');
			} else {
				toasts.error(`Failed to fetch branch: ${err.message}`);
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

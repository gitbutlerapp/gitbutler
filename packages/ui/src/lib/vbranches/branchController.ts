import type { BaseBranch, Hunk } from './types';
import * as toasts from '$lib/utils/toasts';
import { invoke } from '$lib/backend/ipc';
import type { Session } from '$lib/backend/sessions';
import type { BaseBranchService, VirtualBranchService } from './branchStoresCache';
import type { RemoteBranchService } from '$lib/stores/remoteBranches';
import type { Observable } from 'rxjs';

export class BranchController {
	constructor(
		readonly projectId: string,
		readonly virtualBranchStore: VirtualBranchService,
		readonly remoteBranchStore: RemoteBranchService,
		readonly targetBranchStore: BaseBranchService,
		readonly sessionsStore: Observable<Session[]>
	) {}

	async setTarget(branch: string) {
		try {
			await invoke<BaseBranch>('set_base_branch', { projectId: this.projectId, branch });
			this.targetBranchStore.reload();
			// TODO: Reloading seems to trigger 4 invocations of `list_virtual_branches`
		} catch (err) {
			toasts.error('Failed to set base branch');
		} finally {
			this.virtualBranchStore.reload();
		}
	}

	async resetBranch(branchId: string, targetCommitOid: string) {
		try {
			await invoke<void>('reset_virtual_branch', {
				branchId,
				projectId: this.projectId,
				targetCommitOid
			});
		} catch (err) {
			toasts.error('Failed to reset branch');
		} finally {
			this.virtualBranchStore.reload();
		}
	}

	async createBranch(branch: { name?: string; ownership?: string; order?: number }) {
		try {
			await invoke<void>('create_virtual_branch', { projectId: this.projectId, branch });
		} catch (err) {
			toasts.error('Failed to create branch');
		} finally {
			this.virtualBranchStore.reload();
		}
	}

	async commitBranch(branch: string, message: string, ownership: string | undefined = undefined) {
		try {
			await invoke<void>('commit_virtual_branch', {
				projectId: this.projectId,
				branch,
				message,
				ownership
			});
		} catch (err) {
			toasts.error('Failed to commit branch');
		} finally {
			this.virtualBranchStore.reload();
		}
	}

	async mergeUpstream(branch: string) {
		try {
			await invoke<void>('merge_virtual_branch_upstream', { projectId: this.projectId, branch });
			this.virtualBranchStore.reload();
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
		} catch (err) {
			toasts.error('Failed to update branch name');
		} finally {
			this.virtualBranchStore.reload();
		}
	}

	async updateBranchRemoteName(branchId: string, upstream: string) {
		try {
			await invoke<void>('update_virtual_branch', {
				projectId: this.projectId,
				branch: { id: branchId, upstream }
			});
		} catch (err) {
			toasts.error('Failed to update branch remote name');
		} finally {
			this.virtualBranchStore.reload();
		}
	}

	async updateBranchNotes(branchId: string, notes: string) {
		try {
			await invoke<void>('update_virtual_branch', {
				projectId: this.projectId,
				branch: { id: branchId, notes }
			});
		} catch (err) {
			toasts.error('Failed to update branch notes');
		} finally {
			this.virtualBranchStore.reload();
		}
	}

	async updateBranchOrder(branchId: string, order: number) {
		try {
			await invoke<void>('update_virtual_branch', {
				projectId: this.projectId,
				branch: { id: branchId, order }
			});
		} catch (err) {
			toasts.error('Failed to update branch order');
		} finally {
			this.virtualBranchStore.reload();
		}
	}

	async applyBranch(branchId: string) {
		try {
			// TODO: make this optimistic again.
			await invoke<void>('apply_branch', { projectId: this.projectId, branch: branchId });
		} catch (err) {
			toasts.error('Failed to apply branch');
		} finally {
			this.virtualBranchStore.reload();
		}
	}

	async unapplyHunk(hunk: Hunk) {
		const ownership = `${hunk.filePath}:${hunk.id}`;
		try {
			await invoke<void>('unapply_ownership', { projectId: this.projectId, ownership });
		} catch (err) {
			toasts.error('Failed to unapply hunk');
		} finally {
			this.virtualBranchStore.reload();
		}
	}

	async unapplyBranch(branchId: string) {
		try {
			// TODO: make this optimistic again.
			await invoke<void>('unapply_branch', { projectId: this.projectId, branch: branchId });
		} catch (err) {
			toasts.error('Failed to unapply branch');
		} finally {
			this.virtualBranchStore.reload();
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
		} finally {
			this.virtualBranchStore.reload();
		}
	}

	async pushBranch(branchId: string, withForce: boolean) {
		try {
			await invoke<void>('push_virtual_branch', { projectId: this.projectId, branchId, withForce });
		} catch (err: any) {
			if (err.code === 'errors.git.authentication') {
				toasts.error('Failed to authenticate. Did you setup GitButler ssh keys?');
			} else {
				toasts.error(`Failed to push branch: ${err.message}`);
			}
		} finally {
			this.virtualBranchStore.reload();
		}
	}

	async deleteBranch(branchId: string) {
		try {
			// TODO: make this optimistic again.
			await invoke<void>('delete_virtual_branch', { projectId: this.projectId, branchId });
		} catch (err) {
			toasts.error('Failed to delete branch');
		} finally {
			this.virtualBranchStore.reload();
			this.remoteBranchStore.reload();
		}
	}

	async updateBaseBranch() {
		try {
			await invoke<object>('update_base_branch', { projectId: this.projectId });
		} catch (err) {
			toasts.error('Failed to update target');
		} finally {
			this.targetBranchStore.reload();
		}
	}

	async createvBranchFromBranch(branch: string) {
		try {
			await invoke<string>('create_virtual_branch_from_branch', {
				projectId: this.projectId,
				branch
			});
		} catch (err) {
			toasts.error('Failed to create virtual branch from branch');
		} finally {
			this.remoteBranchStore.reload();
			this.virtualBranchStore.reload();
			this.targetBranchStore.reload();
		}
	}

	async fetchFromTarget() {
		try {
			await invoke<void>('fetch_from_target', { projectId: this.projectId });
		} catch (err: any) {
			if (err.code === 'errors.git.authentication') {
				toasts.error('Failed to authenticate. Did you setup GitButler ssh keys?');
			} else {
				toasts.error(`Failed to fetch branch: ${err.message}`);
			}
		} finally {
			this.targetBranchStore.reload();
		}
	}

	async cherryPick(branchId: string, targetCommitOid: string) {
		try {
			await invoke<void>('cherry_pick_onto_virtual_branch', {
				projectId: this.projectId,
				branchId,
				targetCommitOid
			});
		} catch (err: any) {
			toasts.error(`Failed to cherry-pick commit: ${err.message}`);
		} finally {
			this.targetBranchStore.reload();
		}
	}

	async markResolved(path: string) {
		try {
			await invoke<void>('mark_resolved', { projectId: this.projectId, path });
		} catch (err) {
			toasts.error(`Failed to mark file resolved`);
		} finally {
			this.virtualBranchStore.reload();
		}
	}

	async squashBranchCommit(branchId: string, targetCommitOid: string) {
		try {
			await invoke<void>('squash_branch_commit', {
				projectId: this.projectId,
				branchId,
				targetCommitOid
			});
		} catch (err: any) {
			toasts.error(`Failed to squash commit: ${err.message}`);
		} finally {
			this.virtualBranchStore.reload();
		}
	}

	async amendBranch(branchId: string, ownership: string) {
		try {
			await invoke<void>('amend_virtual_branch', {
				projectId: this.projectId,
				branchId,
				ownership
			});
		} catch (err: any) {
			toasts.error(`Failed to amend commit: ${err.message}`);
		} finally {
			this.targetBranchStore.reload();
		}
	}
}

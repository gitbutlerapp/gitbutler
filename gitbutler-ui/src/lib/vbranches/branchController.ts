import { invoke } from '$lib/backend/ipc';
import * as toasts from '$lib/utils/toasts';
import posthog from 'posthog-js';
import type { RemoteBranchService } from '$lib/stores/remoteBranches';
import type { BaseBranchService, VirtualBranchService } from './branchStoresCache';
import type { Branch, Hunk, LocalFile } from './types';

export class BranchController {
	constructor(
		readonly projectId: string,
		readonly vbranchService: VirtualBranchService,
		readonly remoteBranchService: RemoteBranchService,
		readonly targetBranchService: BaseBranchService
	) {}

	async setTarget(branch: string) {
		try {
			await this.targetBranchService.setTarget(branch);
			// TODO: Reloading seems to trigger 4 invocations of `list_virtual_branches`
		} catch (err) {
			toasts.error('Failed to set base branch');
		} finally {
			this.targetBranchService.reload();
			this.vbranchService.reload();
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
		}
	}

	async createBranch(branch: { name?: string; ownership?: string; order?: number }) {
		try {
			await invoke<void>('create_virtual_branch', { projectId: this.projectId, branch });
		} catch (err) {
			toasts.error('Failed to create branch');
		}
	}

	async commitBranch(
		branch: string,
		message: string,
		ownership: string | undefined = undefined,
		runHooks = false
	) {
		try {
			await invoke<void>('commit_virtual_branch', {
				projectId: this.projectId,
				branch,
				message,
				ownership,
				runHooks: runHooks
			});
			posthog.capture('Commit Successful');
		} catch (err) {
			toasts.error('Failed to commit branch');
			posthog.capture('Commit Failed');
		}
	}

	async mergeUpstream(branch: string) {
		try {
			await invoke<void>('merge_virtual_branch_upstream', {
				projectId: this.projectId,
				branch
			});
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
		}
	}

	async setSelectedForChanges(branchId: string) {
		try {
			await invoke<void>('update_virtual_branch', {
				projectId: this.projectId,
				branch: { id: branchId, selected_for_changes: true }
			});
		} catch (err) {
			toasts.error('Failed to set as target');
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
		}
	}

	async applyBranch(branchId: string) {
		try {
			// TODO: make this optimistic again.
			await invoke<void>('apply_branch', { projectId: this.projectId, branch: branchId });
		} catch (err) {
			toasts.error('Failed to apply branch');
		}
	}

	async unapplyHunk(hunk: Hunk) {
		const ownership = `${hunk.filePath}:${hunk.id}`;
		try {
			await invoke<void>('unapply_ownership', { projectId: this.projectId, ownership });
		} catch (err) {
			toasts.error('Failed to unapply hunk');
		}
	}

	async unapplyFiles(files: LocalFile[]) {
		try {
			await invoke<void>('unapply_ownership', {
				projectId: this.projectId,
				ownership: files
					.flatMap((f) => f.hunks.map((h) => `${f.path}:${h.id}`).join('\n'))
					.join('\n')
			});
		} catch (err) {
			toasts.error('Failed to unapply file changes');
		}
	}

	async unapplyBranch(branchId: string) {
		try {
			// TODO: make this optimistic again.
			await invoke<void>('unapply_branch', { projectId: this.projectId, branch: branchId });
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
		} catch (err) {
			toasts.error('Failed to update branch ownership');
		}
	}

	async pushBranch(branchId: string, withForce: boolean): Promise<Branch | undefined> {
		try {
			await invoke<void>('push_virtual_branch', {
				projectId: this.projectId,
				branchId,
				withForce
			});
			await this.vbranchService.reload();
			return await this.vbranchService.getById(branchId);
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
			// TODO: make this optimistic again.
			await invoke<void>('delete_virtual_branch', { projectId: this.projectId, branchId });
			toasts.success('Branch deleted successfully');
		} catch (err) {
			toasts.error('Failed to delete branch');
		} finally {
			this.remoteBranchService.reload();
		}
	}

	async updateBaseBranch() {
		try {
			await invoke<object>('update_base_branch', { projectId: this.projectId });
		} finally {
			this.targetBranchService.reload();
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
			this.remoteBranchService.reload();
			this.targetBranchService.reload();
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
			this.targetBranchService.reload();
		}
	}

	async markResolved(path: string) {
		try {
			await invoke<void>('mark_resolved', { projectId: this.projectId, path });
		} catch (err) {
			toasts.error(`Failed to mark file resolved`);
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
		}
	}
}

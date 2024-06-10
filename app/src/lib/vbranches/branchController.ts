import { invoke } from '$lib/backend/ipc';
import { showError, showToast } from '$lib/notifications/toasts';
import * as toasts from '$lib/utils/toasts';
import posthog from 'posthog-js';
import type { RemoteBranchService } from '$lib/stores/remoteBranches';
import type { BaseBranchService } from './baseBranch';
import type { Branch, Hunk, LocalFile } from './types';
import type { VirtualBranchService } from './virtualBranch';

export class BranchController {
	constructor(
		readonly projectId: string,
		readonly vbranchService: VirtualBranchService,
		readonly remoteBranchService: RemoteBranchService,
		readonly targetBranchService: BaseBranchService
	) {}

	async setTarget(branch: string, pushRemote: string | undefined = undefined) {
		try {
			await this.targetBranchService.setTarget(branch, pushRemote);
			return branch;
			// TODO: Reloading seems to trigger 4 invocations of `list_virtual_branches`
		} catch (err: any) {
			showError('Failed to set base branch', err);
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
			showError('Failed to reset branch', err);
		}
	}

	async createBranch(branch: { name?: string; ownership?: string; order?: number }) {
		try {
			await invoke<void>('create_virtual_branch', { projectId: this.projectId, branch });
		} catch (err) {
			showError('Failed to create branch', err);
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
		} catch (err: any) {
			if (err.code === 'errors.commit.signing_failed') {
				showSignError(err);
			} else {
				showError('Failed to commit changes', err);
			}
			posthog.capture('Commit Failed', err);
			throw err;
		}
	}

	async mergeUpstream(branch: string) {
		try {
			await invoke<void>('integrate_upstream_commits', {
				projectId: this.projectId,
				branch
			});
		} catch (err) {
			showError('Failed to merge upstream branch', err);
		}
	}

	async updateBranchName(branchId: string, name: string) {
		try {
			await invoke<void>('update_virtual_branch', {
				projectId: this.projectId,
				branch: { id: branchId, name }
			});
		} catch (err) {
			showError('Failed to update branch name', err);
		}
	}

	async updateBranchRemoteName(branchId: string, upstream: string) {
		try {
			await invoke<void>('update_virtual_branch', {
				projectId: this.projectId,
				branch: { id: branchId, upstream }
			});
		} catch (err) {
			showError('Failed to update remote name', err);
		}
	}

	async updateBranchNotes(branchId: string, notes: string) {
		try {
			await invoke<void>('update_virtual_branch', {
				projectId: this.projectId,
				branch: { id: branchId, notes }
			});
		} catch (err) {
			showError('Failed to update branch notes', err);
		}
	}

	async setSelectedForChanges(branchId: string) {
		try {
			await invoke<void>('update_virtual_branch', {
				projectId: this.projectId,
				branch: { id: branchId, selected_for_changes: true }
			});
		} catch (err) {
			showError('Failed make default target', err);
		}
	}

	async updateBranchOrder(branchId: string, order: number) {
		try {
			await invoke<void>('update_virtual_branch', {
				projectId: this.projectId,
				branch: { id: branchId, order }
			});
		} catch (err) {
			showError('Failed to update branch order', err);
		}
	}

	async applyBranch(branchId: string) {
		try {
			// TODO: make this optimistic again.
			await invoke<void>('apply_branch', { projectId: this.projectId, branch: branchId });
		} catch (err) {
			showError('Failed to apply branch', err);
		}
	}

	async unapplyHunk(hunk: Hunk) {
		const ownership = `${hunk.filePath}:${hunk.id}-${hunk.hash}`;
		try {
			await invoke<void>('unapply_ownership', { projectId: this.projectId, ownership });
		} catch (err) {
			showError('Failed to unapply hunk', err);
		}
	}

	async unapplyFiles(files: LocalFile[]) {
		try {
			await invoke<void>('reset_files', {
				projectId: this.projectId,
				files: files.flatMap((f) => f.path).join('\n')
			});
		} catch (err) {
			showError('Failed to unapply file changes', err);
		}
	}

	async unapplyBranch(branchId: string) {
		try {
			// TODO: make this optimistic again.
			await invoke<void>('unapply_branch', { projectId: this.projectId, branch: branchId });
			this.remoteBranchService.reload();
		} catch (err) {
			showError('Failed to unapply branch', err);
		}
	}

	async updateBranchOwnership(branchId: string, ownership: string) {
		try {
			await invoke<void>('update_virtual_branch', {
				projectId: this.projectId,
				branch: { id: branchId, ownership }
			});
		} catch (err) {
			showError('Failed to update hunk ownership', err);
		}
	}

	async pushBranch(branchId: string, withForce: boolean): Promise<Branch | undefined> {
		try {
			await invoke<void>('push_virtual_branch', {
				projectId: this.projectId,
				branchId,
				withForce
			});
			posthog.capture('Push Successful');
			await this.vbranchService.reload();
			return await this.vbranchService.getById(branchId);
		} catch (err: any) {
			console.error(err);
			posthog.capture('Push Failed', { error: err });
			if (err.code === 'errors.git.authentication') {
				showToast({
					title: 'Git push failed',
					message: `
                        Your branch cannot be pushed due to an authentication failure.

                        Please check our [documentation](https://docs.gitbutler.com/troubleshooting/fetch-push)
                        on fetching and pushing for ways to resolve the problem.
                    `,
					error: err.message,
					style: 'error'
				});
			} else {
				showToast({
					title: 'Git push failed',
					message: `
                        Your branch cannot be pushed due to an unforeseen problem.

                        Please check our [documentation](https://docs.gitbutler.com/troubleshooting/fetch-push)
                        on fetching and pushing for ways to resolve the problem.
                    `,
					error: err.message,
					style: 'error'
				});
			}
		}
	}

	async deleteBranch(branchId: string) {
		try {
			// TODO: make this optimistic again.
			await invoke<void>('delete_virtual_branch', { projectId: this.projectId, branchId });
			toasts.success('Branch deleted successfully');
		} catch (err) {
			showError('Failed to delete branch', err);
		} finally {
			this.remoteBranchService.reload();
		}
	}

	async updateBaseBranch(): Promise<string | undefined> {
		try {
			const stashedConflicting = await invoke<Branch[]>('update_base_branch', {
				projectId: this.projectId
			});
			if (stashedConflicting.length > 0) {
				return `The following branches were stashed due to a merge conflict during updating the workspace: \n\n \
${stashedConflicting.map((branch) => branch.name).join('\n')} \n\n \
You can find them in the 'Branches' sidebar in order to resolve conflicts.`;
			} else {
				return undefined;
			}
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
			showError('Failed to create virtual branch', err);
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
			// TODO: Probably we wanna have error code checking in a more generic way
			if (err.code === 'errors.commit.signing_failed') {
				showSignError(err);
			} else {
				showError('Failed to cherry-pick commit', err);
			}
		} finally {
			this.targetBranchService.reload();
		}
	}

	async markResolved(path: string) {
		try {
			await invoke<void>('mark_resolved', { projectId: this.projectId, path });
		} catch (err) {
			showError('Failed to mark file resolved', err);
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
			// TODO: Probably we wanna have error code checking in a more generic way
			if (err.code === 'errors.commit.signing_failed') {
				showSignError(err);
			} else {
				showError('Failed to squash commit', err);
			}
		}
	}

	async amendBranch(branchId: string, commitOid: string, ownership: string) {
		try {
			await invoke<void>('amend_virtual_branch', {
				projectId: this.projectId,
				branchId,
				commitOid,
				ownership
			});
		} catch (err: any) {
			showError('Failed to amend commit', err);
		}
	}

	async moveCommitFile(
		branchId: string,
		fromCommitOid: string,
		toCommitOid: string,
		ownership: string
	) {
		try {
			await invoke<void>('move_commit_file', {
				projectId: this.projectId,
				branchId,
				fromCommitOid,
				toCommitOid,
				ownership
			});
		} catch (err: any) {
			showError('Failed to amend commit', err);
		}
	}

	async undoCommit(branchId: string, commitOid: string) {
		try {
			await invoke<void>('undo_commit', {
				projectId: this.projectId,
				branchId,
				commitOid
			});
		} catch (err: any) {
			showError('Failed to amend commit', err);
		}
	}

	async updateCommitMessage(branchId: string, commitOid: string, message: string) {
		try {
			await invoke<void>('update_commit_message', {
				projectId: this.projectId,
				branchId,
				commitOid,
				message
			});
		} catch (err: any) {
			// TODO: Probably we wanna have error code checking in a more generic way
			if (err.code === 'errors.commit.signing_failed') {
				showSignError(err);
			} else {
				showError('Failed to change commit message', err);
			}
		}
	}

	async insertBlankCommit(branchId: string, commitOid: string, offset: number) {
		try {
			await invoke<void>('insert_blank_commit', {
				projectId: this.projectId,
				branchId,
				commitOid,
				offset
			});
		} catch (err: any) {
			// TODO: Probably we wanna have error code checking in a more generic way
			if (err.code === 'errors.commit.signing_failed') {
				showSignError(err);
			} else {
				showError('Failed to insert blank commit', err);
			}
		}
	}

	async reorderCommit(branchId: string, commitOid: string, offset: number) {
		try {
			await invoke<void>('reorder_commit', {
				projectId: this.projectId,
				branchId,
				commitOid,
				offset
			});
		} catch (err: any) {
			// TODO: Probably we wanna have error code checking in a more generic way
			if (err.code === 'errors.commit.signing_failed') {
				showSignError(err);
			} else {
				showError('Failed to reorder blank commit', err);
			}
		}
	}

	async moveCommit(targetBranchId: string, commitOid: string) {
		try {
			await invoke<void>('move_commit', {
				projectId: this.projectId,
				targetBranchId,
				commitOid
			});
		} catch (err: any) {
			// TODO: Probably we wanna have error code checking in a more generic way
			if (err.code === 'errors.commit.signing_failed') {
				showSignError(err);
			} else {
				showError('Failed to move commit', err);
			}
		}
	}
}

function showSignError(err: any) {
	showToast({
		title: 'Failed to commit due to signing error',
		message: `
Signing is now disabled, so subsequent commits will not fail. You can configure commit signing in the project settings.

Please check our [documentation](https://docs.gitbutler.com/features/virtual-branches/verifying-commits) on setting up commit signing and verification.
					`,
		error: err.message,
		style: 'error'
	});
}

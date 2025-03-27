import { invoke } from '$lib/backend/ipc';
import { showError, showToast } from '$lib/notifications/toasts';
import * as toasts from '@gitbutler/ui/toasts';
import type { PostHogWrapper } from '$lib/analytics/posthog';
import type { StackOrder } from '$lib/branches/branch';
import type { BranchListingService } from '$lib/branches/branchListing';
import type { VirtualBranchService } from '$lib/branches/virtualBranchService';
import type { LocalFile } from '$lib/files/file';
import type { TreeChange } from '$lib/hunks/change';
import type { DiffSpec, Hunk } from '$lib/hunks/hunk';

export type CommitIdOrChangeId = { CommitId: string } | { ChangeId: string };
export type SeriesIntegrationStrategy = 'merge' | 'rebase' | 'hardreset';

export class BranchController {
	constructor(
		private readonly projectId: string,
		private readonly vbranchService: VirtualBranchService,
		private readonly branchListingService: BranchListingService,
		private readonly posthog: PostHogWrapper
	) {}

	/**
	 * @deprecated
	 */
	async resetBranch(stackId: string, targetCommitOid: string) {
		try {
			await invoke<void>('reset_virtual_branch', {
				stackId,
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

	async runHooks(stackId: string, ownership: string) {
		await invoke<void>('run_hooks', {
			projectId: this.projectId,
			stackId,
			ownership
		});
	}

	/**
	 * @deprecated
	 */
	async commit(stackId: string, message: string, ownership: string | undefined = undefined) {
		try {
			await invoke<void>('commit_virtual_branch', {
				projectId: this.projectId,
				stackId,
				message,
				ownership
			});
			this.posthog.capture('Commit Successful');
		} catch (err: any) {
			showError('Failed to commit changes', err);
			this.posthog.capture('Commit Failed', err);
		}
	}

	async integrateUpstreamForSeries(
		stackId: string,
		seriesName: string,
		strategy?: SeriesIntegrationStrategy
	) {
		const integrationStrategy = strategy ? { type: strategy } : undefined;
		try {
			await invoke<void>('integrate_upstream_commits', {
				projectId: this.projectId,
				stackId,
				seriesName,
				integrationStrategy
			});
		} catch (err) {
			showError('Failed to merge upstream branch', err);
		}
	}

	/**
	 * @note - Ported to redux
	 */
	async createPatchSeries(
		stackId: string,
		referenceName: string,
		commitIdOrChangeId?: CommitIdOrChangeId
	) {
		try {
			await invoke<void>('create_branch', {
				projectId: this.projectId,
				stackId,
				request: {
					target_patch: commitIdOrChangeId,
					name: referenceName
				}
			});
		} catch (err) {
			showError('Failed to create branch reference', err);
		}
	}

	/**
	 * @deprecated
	 */
	async removePatchSeries(stackId: string, branchName: string) {
		try {
			await invoke<void>('remove_branch', {
				projectId: this.projectId,
				stackId,
				branchName
			});
		} catch (err) {
			showError('Failed remove series', err);
		}
	}

	/**
	 * Updates the name of the series and resets the forge id to undefined.
	 * @note - Ported to redux
	 */
	async updateBranchName(stackId: string, branchName: string, newBranchName: string) {
		try {
			await invoke<void>('update_branch_name', {
				projectId: this.projectId,
				stackId,
				branchName,
				newBranchName
			});
		} catch (err) {
			showError('Failed to update remote name', err);
		}
	}

	/**
	 * Updates the forge identifier for a branch/series.
	 * This is useful for storing for example the Pull Request Number for a branch.
	 *
	 * @note - Ported to redux
	 */
	async updateBranchPrNumber(stackId: string, branchName: string, prNumber: number | null) {
		try {
			await invoke<void>('update_branch_pr_number', {
				projectId: this.projectId,
				stackId,
				branchName,
				prNumber
			});
		} catch (err) {
			showError('Failed to update pr number', err);
		}
	}

	/**
	 * Updates the series description.
	 *
	 * @note - Ported to redux
	 */
	async updateSeriesDescription(stackId: string, branchName: string, description: string) {
		try {
			await invoke<void>('update_branch_description', {
				projectId: this.projectId,
				stackId,
				branchName,
				description
			});
		} catch (err) {
			showError('Failed to update series description', err);
		}
	}

	/**
	 * @note - Ported to redux
	 */
	async reorderStackCommit(stackId: string, stackOrder: StackOrder) {
		try {
			await invoke<void>('reorder_stack', {
				projectId: this.projectId,
				stackId,
				stackOrder
			});
		} catch (err) {
			showError('Failed to reorder stack commit', err);
		}
	}

	/**
	 * @deprecated
	 */
	async updateBranchRemoteName(stackId: string, upstream: string) {
		try {
			await invoke<void>('update_virtual_branch', {
				projectId: this.projectId,
				branch: { id: stackId, upstream }
			});
		} catch (err) {
			showError('Failed to update remote name', err);
		}
	}

	async updateBranchAllowRebasing(stackId: string, allowRebasing: boolean) {
		try {
			await invoke<void>('update_virtual_branch', {
				projectId: this.projectId,
				branch: { id: stackId, allow_rebasing: allowRebasing }
			});
		} catch (err) {
			showError('Failed to update branch allow rebasing', err);
		}
	}

	async updateBranchNotes(stackId: string, notes: string) {
		try {
			await invoke<void>('update_virtual_branch', {
				projectId: this.projectId,
				branch: { id: stackId, notes }
			});
		} catch (err) {
			showError('Failed to update branch notes', err);
		}
	}

	async setSelectedForChanges(stackId: string) {
		try {
			await invoke<void>('update_virtual_branch', {
				projectId: this.projectId,
				branch: { id: stackId, selected_for_changes: true }
			});
		} catch (err) {
			showError('Failed make default target', err);
		}
	}

	async updateBranchOrder(branches: { id: string; order: number }[]) {
		try {
			await invoke<void>('update_branch_order', {
				projectId: this.projectId,
				branches
			});
		} catch (err) {
			showError('Failed to update branch order', err);
		}
	}

	/**
	 * @deprecated
	 */
	async unapplyLines(hunk: Hunk, linesToUnapply: { old?: number; new?: number }[]) {
		const ownership = `${hunk.filePath}:${hunk.id}-${hunk.hash}`;
		const lines = {
			[hunk.id]: linesToUnapply
		};
		try {
			await invoke<void>('unapply_lines', { projectId: this.projectId, ownership, lines });
		} catch (err) {
			showError('Failed to unapply lines', err);
		}
	}

	/**
	 * @deprecated
	 */
	async unapplyHunk(hunk: Hunk) {
		const ownership = `${hunk.filePath}:${hunk.id}-${hunk.hash}`;
		try {
			await invoke<void>('unapply_ownership', { projectId: this.projectId, ownership });
		} catch (err) {
			showError('Failed to unapply hunk', err);
		}
	}

	/**
	 * @deprecated
	 */
	async unapplyFiles(stackId: string, files: LocalFile[]) {
		try {
			await invoke<void>('reset_files', {
				projectId: this.projectId,
				stackId,
				files: files?.flatMap((f) => f.path) ?? []
			});
		} catch (err) {
			showError('Failed to unapply file changes', err);
		}
	}

	/**
	 * @deprecated
	 */
	async unapplyChanges(stackId: string | undefined, changes: TreeChange[]) {
		// TODO: this won't for changes.
		// There are some changes required on the rust side to make this work.
		try {
			await invoke<void>('reset_files', {
				projectId: this.projectId,
				stackId,
				files: changes.map((c) => c.path)
			});
		} catch (err) {
			showError('Failed to unapply changes', err);
		}
	}

	/**
	 * @note Ported to redux
	 */
	async saveAndUnapply(stackId: string) {
		try {
			await invoke<void>('save_and_unapply_virtual_branch', {
				projectId: this.projectId,
				stackId
			});
			this.branchListingService.refresh();
		} catch (err) {
			showError('Failed to unapply branch', err);
		}
	}

	/**
	 * @deprecated
	 */
	async updateBranchOwnership(stackId: string, ownership: string) {
		try {
			await invoke<void>('update_virtual_branch', {
				projectId: this.projectId,
				branch: { id: stackId, ownership }
			});
		} catch (err) {
			showError('Failed to update hunk ownership', err);
		}
	}

	/**
	 * @note - Ported to redux
	 */
	async pushBranch(stackId: string, withForce: boolean): Promise<BranchPushResult | undefined> {
		try {
			const pushResult = await invoke<BranchPushResult | undefined>('push_stack', {
				projectId: this.projectId,
				stackId,
				withForce
			});
			this.posthog.capture('Push Successful');
			await this.vbranchService.refresh();
			return pushResult;
		} catch (err: any) {
			console.error(err);
			const { code, message } = err;
			this.posthog.capture('Push Failed', { error: { code, message } });

			if (code === 'errors.git.authentication') {
				showToast({
					title: 'Git push failed',
					message: `
                        Your branch cannot be pushed due to an authentication failure.

                        Please check our [documentation](https://docs.gitbutler.com/troubleshooting/fetch-push)
                        on fetching and pushing for ways to resolve the problem.
                    `,
					error: message,
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
					error: message,
					style: 'error'
				});
			}
			throw err;
		}
	}

	/**
	 * @note - Ported to redux
	 */
	async unapplyWithoutSaving(stackId: string) {
		try {
			// TODO: make this optimistic again.
			await invoke<void>('unapply_without_saving_virtual_branch', {
				projectId: this.projectId,
				stackId
			});
			toasts.success('Branch unapplied successfully');
		} catch (err) {
			showError('Failed to unapply branch', err);
		} finally {
			this.branchListingService.refresh();
		}
	}

	/**
	 *
	 * @param branch The branch you want to create a virtual branch for. If you
	 * have a local branch, this should be the branch.
	 * @param remote Optionally sets another branch as the upstream.
	 */
	async createvBranchFromBranch(
		branch: string,
		remote: string | undefined = undefined,
		prNumber: number | undefined = undefined
	) {
		try {
			await invoke<string>('create_virtual_branch_from_branch', {
				projectId: this.projectId,
				branch,
				remote,
				prNumber
			});
		} catch (err) {
			showError('Failed to create virtual branch', err);
		} finally {
			this.branchListingService.refresh();
		}
	}

	/**
	 * Removes a branch local reference and any associated virtual branch if applicable and updates the list of branches know to the UI.
	 * @param branch The reference name of the branch to delete (including the `refs/heads/` prefix).
	 */
	async deleteLocalBranch(refname: string, givenName: string) {
		try {
			await invoke<void>('delete_local_branch', {
				projectId: this.projectId,
				refname,
				givenName
			});
		} catch (err) {
			showError('Failed to delete local branch', err);
		} finally {
			this.branchListingService.refresh();
		}
	}

	async markResolved(path: string) {
		try {
			await invoke<void>('mark_resolved', { projectId: this.projectId, path });
		} catch (err) {
			showError('Failed to mark file resolved', err);
		}
	}

	async squashBranchCommit(stackId: string, sourceCommitOid: string, targetCommitOid: string) {
		try {
			await invoke<void>('squash_commits', {
				projectId: this.projectId,
				stackId,
				sourceCommitOids: [sourceCommitOid], // The API has the ability to squash multiple commits, but currently the UI only squashes one at a time
				targetCommitOid
			});
		} catch (err: any) {
			showError('Failed to squash commit', err);
		}
	}

	async amendBranch(stackId: string, commitId: string, worktreeChanges: DiffSpec[]) {
		try {
			await invoke<void>('amend_virtual_branch', {
				projectId: this.projectId,
				stackId,
				commitId,
				worktreeChanges
			});
		} catch (err: any) {
			showError('Failed to amend commit', err);
		}
	}

	async moveCommitFile(
		stackId: string,
		fromCommitOid: string,
		toCommitOid: string,
		ownership: string
	) {
		try {
			await invoke<void>('move_commit_file', {
				projectId: this.projectId,
				stackId,
				fromCommitOid,
				toCommitOid,
				ownership
			});
		} catch (err: any) {
			showError('Failed to amend commit', err);
		}
	}

	/**
	 * @note - Ported to redux
	 */
	async undoCommit(stackId: string, branchName: string, commitOid: string) {
		try {
			await invoke<void>('undo_commit', {
				projectId: this.projectId,
				stackId,
				commitOid
			});
		} catch (err: any) {
			showError('Failed to amend commit', err);
		}
	}

	/**
	 * @note - Ported to redux
	 */
	async updateCommitMessage(stackId: string, commitOid: string, message: string) {
		try {
			await invoke<void>('update_commit_message', {
				projectId: this.projectId,
				stackId,
				commitOid,
				message
			});
		} catch (err: any) {
			showError('Failed to change commit message', err);
		}
	}

	/**
	 * @note - Ported to redux
	 */
	async insertBlankCommit(stackId: string, commitOid: string, offset: number) {
		try {
			await invoke<void>('insert_blank_commit', {
				projectId: this.projectId,
				stackId,
				commitOid,
				offset
			});
		} catch (err: any) {
			showError('Failed to insert blank commit', err);
		}
	}

	/**
	 * @note - Ported to redux
	 */
	async moveCommit(targetStackId: string, commitOid: string, sourceStackId: string) {
		try {
			await invoke<void>('move_commit', {
				projectId: this.projectId,
				targetStackId,
				commitOid,
				sourceStackId
			});
		} catch (err: any) {
			showError('Failed to move commit', err);
		}
	}
}
export interface BranchPushResult {
	refname: string;
	remote: string;
}

import { invoke } from '$lib/backend/ipc';
import { showError } from '$lib/notifications/toasts';
import type { PostHogWrapper } from '$lib/analytics/posthog';
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
}
export interface BranchPushResult {
	refname: string;
	remote: string;
}

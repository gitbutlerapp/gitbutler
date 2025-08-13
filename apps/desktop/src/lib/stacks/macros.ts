import { getStackName } from '$lib/stacks/stack';
import type { DiffSpec } from '$lib/hunks/hunk';
import type {
	CreateCommitRequestWorktreeChanges,
	RejectionReason,
	StackService
} from '$lib/stacks/stackService.svelte';
import type { UiState } from '$lib/state/uiState.svelte';

const STUB_COMMIT_MESSAGE = 'New commit';

export type BranchChangesParams = {
	branchName?: string;
	commitMessage?: string;
	worktreeChanges: CreateCommitRequestWorktreeChanges[];
};

export default class StackMacros {
	constructor(
		private readonly projectId: string,
		private readonly stackService: StackService,
		private readonly uiState: UiState
	) {}

	/**
	 * Creates a new stack and commit with the given changes.
	 *
	 * After the successful creation, the stack and commit are selected in the UI state.
	 *
	 * Optionally, a branch name and commit message can be provided.
	 */
	async branchChanges(params: BranchChangesParams) {
		const { stack, outcome, branchName } = await this.createNewStackAndCommit(
			params.worktreeChanges,
			params.branchName,
			params.commitMessage
		);
		if (!stack.id) return;
		if (outcome.newCommit) {
			this.uiState.lane(stack.id).selection.set({
				branchName,
				commitId: outcome.newCommit
			});

			this.uiState.project(this.projectId).stackId.set(stack.id);
		}
	}

	/**
	 * Creates a new stack and stub commit into it.
	 */
	async createNewStackAndCommit(
		worktreeChanges: CreateCommitRequestWorktreeChanges[] = [],
		name?: string,
		message?: string
	) {
		const stack = await this.stackService.newStackMutation({
			projectId: this.projectId,
			branch: { name }
		});
		if (!stack.id) {
			throw new Error('New stack has no stack id');
		}
		const branchName = getStackName(stack);
		const outcome = await this.stackService.createCommitMutation({
			projectId: this.projectId,
			stackId: stack.id,
			stackBranchName: branchName,
			parentId: undefined,
			message: message ?? STUB_COMMIT_MESSAGE,
			worktreeChanges
		});

		if (outcome.pathsToRejectedChanges.length > 0) {
			const pathsToRejectedChanges = outcome.pathsToRejectedChanges.reduce(
				(acc: Record<string, RejectionReason>, [reason, path]) => {
					acc[path] = reason;
					return acc;
				},
				{}
			);

			this.uiState.global.modal.set({
				type: 'commit-failed',
				projectId: this.projectId,
				targetBranchName: branchName,
				newCommitId: outcome.newCommit ?? undefined,
				commitTitle: message ?? STUB_COMMIT_MESSAGE,
				pathsToRejectedChanges
			});
		}
		return { stack, outcome, branchName };
	}

	/**
	 * Moves the changes from the source commit to the new commit in the new stack.
	 */
	async moveChangesToNewCommit(
		destinationStackId: string,
		destinationCommitId: string,
		sourceStackId: string,
		sourceCommitId: string,
		branchName: string,
		changes: DiffSpec[]
	) {
		const { replacedCommits } = await this.stackService.moveChangesBetweenCommits({
			projectId: this.projectId,
			destinationStackId: destinationStackId,
			destinationCommitId: destinationCommitId,
			sourceStackId,
			sourceCommitId,
			changes
		});

		const newCommitId = replacedCommits.find(([before]) => before === destinationCommitId)?.[1];
		if (!newCommitId) {
			// This happend only if something went wrong
			throw new Error('No new commit id found for the moved changes');
		}

		this.uiState.lane(destinationStackId).selection.set({
			branchName,
			commitId: newCommitId
		});
		this.uiState.project(this.projectId).stackId.set(destinationStackId);
	}
}

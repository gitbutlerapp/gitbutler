import {
	rejectedChangesByPath,
	toCommitCreatePlacement,
	type CreateCommitRequestWorktreeChanges,
} from "$lib/stacks/stackEndpoints";
import type { StackService } from "$lib/stacks/stackService.svelte";
import type { UiState } from "$lib/state/uiState.svelte";
import type { DiffSpec } from "@gitbutler/but-sdk";

const STUB_COMMIT_MESSAGE = "New commit";

export type BranchChangesParams = {
	branchName?: string;
	commitMessage?: string;
	worktreeChanges: CreateCommitRequestWorktreeChanges[];
};

export default class StackMacros {
	constructor(
		private readonly projectId: string,
		private readonly stackService: StackService,
		private readonly uiState: UiState,
	) {}

	/**
	 * Creates a new stack and commit with the given changes.
	 *
	 * After the successful creation, the stack and commit are selected in the UI state.
	 *
	 * Optionally, a branch name and commit message can be provided.
	 */
	async branchChanges(params: BranchChangesParams) {
		const { stackId, outcome, branchName } = await this.createNewStackAndCommit(
			params.worktreeChanges,
			params.branchName,
			params.commitMessage,
		);
		if (outcome.newCommit) {
			this.uiState.lane(stackId).selection.set({
				branchName,
				commitId: outcome.newCommit,
				previewOpen: true,
			});
		}
	}

	/**
	 * Creates a new stack and stub commit into it.
	 */
	async createNewStackAndCommit(
		worktreeChanges: CreateCommitRequestWorktreeChanges[] = [],
		name?: string,
		message?: string,
	) {
		const normalizedName = name ? await this.stackService.normalizeBranchName(name) : undefined;
		const created = await this.stackService.branchCreateMutation({
			projectId: this.projectId,
			newRef: normalizedName ? `refs/heads/${normalizedName}` : null,
			placement: { type: "independent", subject: { order: null } },
		});
		const branchName = created.newRef.displayName;
		const stack = created.workspace.headInfo.stacks.find((stack) =>
			stack.segments.some((segment) => segment.refName?.displayName === branchName),
		);
		if (!stack?.id) throw new Error("Created branch has no workspace stack");
		const outcome = await this.stackService.createCommitMutation({
			projectId: this.projectId,
			...toCommitCreatePlacement({ stackBranchName: branchName }),
			message: message ?? STUB_COMMIT_MESSAGE,
			changes: worktreeChanges,
		});

		if (outcome.rejectedChanges.length > 0) {
			this.uiState.global.modal.set({
				type: "commit-failed",
				projectId: this.projectId,
				targetBranchName: branchName,
				newCommitId: outcome.newCommit ?? undefined,
				commitTitle: message ?? STUB_COMMIT_MESSAGE,
				pathsToRejectedChanges: rejectedChangesByPath(outcome),
			});
		}
		return { stackId: stack.id, outcome, branchName };
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
		changes: DiffSpec[],
	) {
		const { workspace } = await this.stackService.moveChangesBetweenCommits({
			projectId: this.projectId,
			destinationStackId: destinationStackId,
			destinationCommitId: destinationCommitId,
			sourceStackId,
			sourceCommitId,
			changes,
			dryRun: false,
		});

		const newCommitId = workspace.replacedCommits[destinationCommitId];
		if (!newCommitId) {
			// This happened only if something went wrong
			throw new Error("No new commit id found for the moved changes");
		}

		const previewOpen =
			this.uiState.lane(destinationStackId).selection.current?.previewOpen ?? false;
		this.uiState.lane(destinationStackId).selection.set({
			branchName,
			commitId: newCommitId,
			previewOpen,
		});
	}
}

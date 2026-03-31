import type {
	ExclusiveAction,
	WritableReactiveStore,
	ProjectUiState,
	StackSelection,
	UiState,
} from "$lib/state/uiState.svelte";
import type { StackDetails } from "@gitbutler/but-sdk";

export function replaceBranchInExclusiveAction(
	action: ExclusiveAction,
	oldBranchName: string,
	branchName: string,
): ExclusiveAction {
	switch (action.type) {
		case "commit":
			if (action.branchName === oldBranchName) {
				return { ...action, branchName };
			}
			return action;
		case "edit-commit-message":
			return action; // No change needed
		case "create-pr":
			if (action.branchName === oldBranchName) {
				return { ...action, branchName };
			}
			return action;
		case "codegen":
			return action;
	}
}

export function replaceBranchInStackSelection(
	selection: StackSelection,
	oldBranchName: string,
	branchName: string,
): StackSelection {
	if (selection.branchName === oldBranchName) {
		return { ...selection, branchName };
	}
	return selection;
}

/**
 * Updates the stack selection to reflect the current state of branches and commits.
 *
 * Pass `prevDetails` so the function can distinguish between a commit being amended
 * (SHA changed, same position — update the selection) vs. deleted/squashed (clear it).
 */
export function updateStackSelection(
	uiState: UiState,
	stackId: string,
	details: StackDetails,
	prevDetails?: StackDetails,
): void {
	const laneState = uiState.lane(stackId);
	const selection = laneState.selection.current;
	const branches = details.branchDetails.map((branch) => branch.name);

	// If no selection, do nothing
	if (!selection) return;

	// Clear selection if the selected branch is not in the list of branches
	if (selection.branchName && !branches.includes(selection.branchName)) {
		laneState.selection.set(undefined);
		return;
	}

	// If the selected branch exists and there is no commit selected, do nothing
	if (!selection.commitId) return;

	const selectedBranch = selection.branchName;
	const branchDetails = details.branchDetails.find((branch) => branch.name === selectedBranch);

	if (!branchDetails) {
		// Should not happen since we already checked the branch exists
		return;
	}

	const branchCommits = branchDetails.commits;
	const branchCommitIds = branchCommits.map((commit) => commit.id);

	// If the selected commit is not in the branch, it may have been amended (SHA changed)
	// or deleted/squashed. Distinguish between the two using the previous state.
	if (!selection.upstream && !branchCommitIds.includes(selection.commitId)) {
		const prevBranchDetails = prevDetails?.branchDetails.find(
			(branch) => branch.name === selectedBranch,
		);
		const oldIndex = prevBranchDetails?.commits.findIndex(
			(commit) => commit.id === selection.commitId,
		);

		// If the commit existed at position N, the count stayed the same, and position N still
		// exists, the commit was amended (same position, new SHA). Update the selection.
		// A count decrease means the commit was truly deleted or squashed — clear it instead.
		const sameCount = (prevBranchDetails?.commits.length ?? 0) === branchCommits.length;
		if (oldIndex !== undefined && oldIndex !== -1 && oldIndex < branchCommits.length && sameCount) {
			laneState.selection.set({ ...selection, commitId: branchCommits[oldIndex]!.id });
			return;
		}

		// Commit is truly gone (deleted, squashed) — clear just the commitId
		laneState.selection.set({
			branchName: selection.branchName,
			previewOpen: false,
		});

		return;
	}

	const upstreamCommits = branchDetails.upstreamCommits;
	const upstreamCommitIds = upstreamCommits.map((commit) => commit.id);

	// If the selection is for an upstream commit and the commit is not in the upstream commits, clear the selection
	if (selection.upstream && !upstreamCommitIds.includes(selection.commitId)) {
		laneState.selection.set({
			branchName: selection.branchName,
			previewOpen: false,
		});

		return;
	}
}

/**
 * Update the project state based on the current stacks, branches and commits.
 *
 * - Clears the selected stack if it no longer exists.
 * - Clears the exclusive action if it references a non-existing stack, branch or commit.
 */
export function updateStaleProjectState(
	uiState: UiState,
	projectId: string,
	stackIds: string[],
	branches: string[],
	commitIds: string[],
	baseCommitShas: string[],
) {
	const projectState = uiState.project(projectId);

	if (projectState.exclusiveAction.current) {
		updateExclusiveActionState(
			projectState.exclusiveAction.current,
			projectState,
			stackIds,
			commitIds,
			branches,
			baseCommitShas,
		);
	}
}

function updateExclusiveActionState(
	action: ExclusiveAction,
	projectState: WritableReactiveStore<ProjectUiState>,
	stackIds: string[],
	commitIds: string[],
	branches: string[],
	baseCommitShas: string[],
) {
	switch (action.type) {
		case "commit":
			if (
				(action.stackId && !stackIds.includes(action.stackId)) ||
				(action.parentCommitId &&
					!commitIds.includes(action.parentCommitId) &&
					!baseCommitShas.includes(action.parentCommitId)) ||
				(action.branchName && !branches.includes(action.branchName))
			) {
				projectState.exclusiveAction.set(undefined);
			}
			break;
		case "edit-commit-message":
			if (
				(action.stackId && !stackIds.includes(action.stackId)) ||
				(action.commitId && !commitIds.includes(action.commitId)) ||
				(action.branchName && !branches.includes(action.branchName))
			) {
				projectState.exclusiveAction.set(undefined);
			}
			break;
		case "create-pr":
			if (
				(action.stackId && !stackIds.includes(action.stackId)) ||
				(action.branchName && !branches.includes(action.branchName))
			) {
				projectState.exclusiveAction.set(undefined);
			}
			break;
	}
}

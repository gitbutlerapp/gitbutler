import type { StackDetails } from "$lib/stacks/stack";
import type {
	ExclusiveAction,
	GlobalStore,
	ProjectUiState,
	StackSelection,
	UiState,
} from "$lib/state/uiState.svelte";

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

function updateStackSelection(uiState: UiState, stackId: string, details: StackDetails): void {
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

	// If the selected commit is not in the branch, clear the commit selection
	if (!selection.upstream && !branchCommitIds.includes(selection.commitId)) {
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
 * Updates the current stack state selection and exclusive action.
 */
export function updateStaleStackState(
	uiState: UiState,
	stackId: string,
	details: StackDetails,
): void {
	updateStackSelection(uiState, stackId, details);
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
	projectState: GlobalStore<ProjectUiState>,
	stackIds: string[],
	commitIds: string[],
	branches: string[],
	baseCommitShas: string[],
) {
	switch (action.type) {
		case "commit":
			if (action.stackId && !stackIds.includes(action.stackId)) {
				projectState.exclusiveAction.set(undefined);
			}
			if (
				action.parentCommitId &&
				!commitIds.includes(action.parentCommitId) &&
				!baseCommitShas.includes(action.parentCommitId)
			) {
				projectState.exclusiveAction.set(undefined);
			}
			if (action.branchName && !branches.includes(action.branchName)) {
				projectState.exclusiveAction.set(undefined);
			}
			break;
		case "edit-commit-message":
			if (action.stackId && !stackIds.includes(action.stackId)) {
				projectState.exclusiveAction.set(undefined);
			}
			if (action.commitId && !commitIds.includes(action.commitId)) {
				projectState.exclusiveAction.set(undefined);
			}
			if (action.branchName && !branches.includes(action.branchName)) {
				projectState.exclusiveAction.set(undefined);
			}
			break;
		case "create-pr":
			if (action.stackId && !stackIds.includes(action.stackId)) {
				projectState.exclusiveAction.set(undefined);
			}
			if (action.branchName && !branches.includes(action.branchName)) {
				projectState.exclusiveAction.set(undefined);
			}
			break;
	}
}

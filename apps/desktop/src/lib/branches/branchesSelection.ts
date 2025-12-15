import type { GlobalProperty } from '$lib/state/uiState.svelte';
import type { BranchesSelection } from '$lib/state/uiState.svelte';

/**
 * Centralized functions for managing branches selection state.
 *
 * Direct calls to set() in the uiState for branchesSelection should be avoided because
 * in the past this led to dropping of important state, e.g. dropping the "inWorkspace" field
 * when selecting a commit in a branch.
 */
export const BranchesSelectionActions = {
	selectStack(
		state: GlobalProperty<BranchesSelection>,
		params: {
			stackId: string;
			branchName: string;
			inWorkspace: boolean;
			hasLocal: boolean;
			prNumber?: number;
		}
	): void {
		state.set({
			stackId: params.stackId,
			branchName: params.branchName,
			inWorkspace: params.inWorkspace,
			hasLocal: params.hasLocal,
			prNumber: params.prNumber
		});
	},

	selectBranch(
		state: GlobalProperty<BranchesSelection>,
		params: {
			branchName: string;
			hasLocal: boolean;
			remote?: string;
			prNumber?: number;
		}
	): void {
		state.set({
			branchName: params.branchName,
			hasLocal: params.hasLocal,
			remote: params.remote,
			prNumber: params.prNumber
		});
	},

	/**
	 * Preserves existing context (stackId, branchName, inWorkspace, etc.) and only updates commitId.
	 */
	selectCommit(
		state: GlobalProperty<BranchesSelection>,
		params: {
			commitId: string;
			remote?: string;
		}
	): void {
		state.update({
			commitId: params.commitId,
			...(params.remote !== undefined ? { remote: params.remote } : {})
		});
	},

	selectTarget(state: GlobalProperty<BranchesSelection>, branchName: string): void {
		state.set({
			branchName,
			isTarget: true
		});
	},

	/**
	 * Selects a PR by number only (no associated branch).
	 */
	selectPr(state: GlobalProperty<BranchesSelection>, prNumber: number): void {
		state.set({ prNumber });
	},

	selectTargetCommit(
		state: GlobalProperty<BranchesSelection>,
		params: {
			commitId: string;
			branchName: string;
			remote?: string;
		}
	): void {
		state.set({
			commitId: params.commitId,
			branchName: params.branchName,
			remote: params.remote
		});
	},

	clear(state: GlobalProperty<BranchesSelection>): void {
		state.set({});
	}
};

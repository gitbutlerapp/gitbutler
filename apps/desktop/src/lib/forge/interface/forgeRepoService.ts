import type { ReactiveQuery } from "$lib/state/butlerModule";

export interface RepoDetailedInfo {
	/**
	 * Whether the repository will delete the branch after merging the PR.
	 *
	 * `undefined` if unknown.
	 */
	deleteBranchAfterMerge: boolean | undefined;
	/** Whether this repository is a fork of another. */
	fork: boolean;
	/** Caller's write permission on the repo (used to gate the merge button). */
	canMerge: boolean;
}

export type ForgeRepoService = {
	getInfo(projectId: string): ReactiveQuery<RepoDetailedInfo>;
};

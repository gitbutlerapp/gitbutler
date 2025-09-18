import type { ReactiveQuery } from '$lib/state/butlerModule';

export interface RepoDetailedInfo {
	/**
	 * Whether the repository will delete the branch after merging the PR.
	 *
	 * `undefined` if unknown.
	 */
	deleteBranchAfterMerge: boolean | undefined;
}

export type ForgeRepoService = {
	getInfo(): ReactiveQuery<RepoDetailedInfo>;
};

import { buildContextStore } from '@gitbutler/shared/context';
import type { Readable } from 'svelte/store';

export interface RepoDetailedInfo {
	/**
	 * Whether the repository will delete the branch after merging the PR.
	 *
	 * `undefined` if unknown.
	 */
	deleteBranchAfterMerge: boolean | undefined;
}

export const [getForgeRepoService, createForgeRepoServiceStore] = buildContextStore<
	ForgeRepoService | undefined
>('forgeRepoService');

export interface ForgeRepoService {
	info: Readable<RepoDetailedInfo | undefined>;
}

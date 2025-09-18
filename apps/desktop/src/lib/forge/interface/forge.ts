import type { ForgeBranch } from '$lib/forge/interface/forgeBranch';
import type { ChecksService } from '$lib/forge/interface/forgeChecksMonitor';
import type { ForgeIssueService } from '$lib/forge/interface/forgeIssueService';
import type { ForgeListingService } from '$lib/forge/interface/forgeListingService';
import type { ForgePrService } from '$lib/forge/interface/forgePrService';
import type { ForgeRepoService } from '$lib/forge/interface/forgeRepoService';
import type { ForgeUser } from '$lib/forge/interface/types';
import type { ReactiveQuery } from '$lib/state/butlerModule';
import type { ReduxTag } from '$lib/state/tags';
import type { PayloadAction } from '@reduxjs/toolkit';
import type { TagDescription } from '@reduxjs/toolkit/query';

export type ForgeName = 'github' | 'gitlab' | 'bitbucket' | 'azure' | 'default';

export interface Forge {
	readonly name: ForgeName;
	readonly authenticated: boolean;
	// Lists PRs for the repo.
	get listService(): ForgeListingService | undefined;

	get issueService(): ForgeIssueService | undefined;

	// Detailed information about a specific PR.
	get prService(): ForgePrService | undefined;

	// Detailed information about the repo.
	get repoService(): ForgeRepoService | undefined;

	// Results from CI check-runs.
	get checks(): ChecksService | undefined;

	get user(): ReactiveQuery<ForgeUser>;

	// Host specific branch information.
	branch(name: string): ForgeBranch | undefined;

	// Web URL for a commit.
	commitUrl(id: string): string | undefined;

	invalidate(tags: TagDescription<ReduxTag>[]): PayloadAction<any> | undefined;
}

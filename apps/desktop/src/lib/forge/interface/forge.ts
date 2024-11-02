import { buildContextStore } from '@gitbutler/shared/context';
import type { ForgeIssueService } from '$lib/forge/interface/forgeIssueService';
import type { ForgeBranch } from './forgeBranch';
import type { ForgeChecksMonitor } from './forgeChecksMonitor';
import type { ForgeListingService } from './forgeListingService';
import type { ForgePrService } from './forgePrService';
import type { ForgeName } from './types';

export interface Forge {
	readonly name: ForgeName;
	// Lists PRs for the repo.
	listService(): ForgeListingService | undefined;

	issueService(): ForgeIssueService | undefined;

	// Detailed information about a specific PR.
	prService(): ForgePrService | undefined;

	// Results from CI check-runs.
	checksMonitor(branchName: string): ForgeChecksMonitor | undefined;

	// Host specific branch information.
	branch(name: string): ForgeBranch | undefined;

	// Web URL for a commit.
	commitUrl(id: string): string;
}

export const [getForge, createForgeStore] = buildContextStore<Forge | undefined>('githubService');

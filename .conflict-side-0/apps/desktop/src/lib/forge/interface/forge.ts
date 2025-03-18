import type { ForgeIssueService } from '$lib/forge/interface/forgeIssueService';
import type { ForgeBranch } from './forgeBranch';
import type { ForgeChecksMonitor } from './forgeChecksMonitor';
import type { ForgeListingService } from './forgeListingService';
import type { ForgePrService } from './forgePrService';
import type { ForgeRepoService } from './forgeRepoService';

export type ForgeName = 'github' | 'gitlab' | 'bitbucket' | 'azure' | 'default';

export interface Forge {
	readonly name: ForgeName;
	// Lists PRs for the repo.
	get listService(): ForgeListingService | undefined;

	get issueService(): ForgeIssueService | undefined;

	// Detailed information about a specific PR.
	get prService(): ForgePrService | undefined;

	// Detailed information about the repo.
	get repoService(): ForgeRepoService | undefined;

	// Results from CI check-runs.
	checksMonitor(branchName: string): ForgeChecksMonitor | undefined;

	// Host specific branch information.
	branch(name: string): ForgeBranch | undefined;

	// Web URL for a commit.
	commitUrl(id: string): string | undefined;
}

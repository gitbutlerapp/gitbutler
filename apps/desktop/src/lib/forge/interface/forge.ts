import type { ForgeBranch } from '$lib/forge/interface/forgeBranch';
import type { ChecksService } from '$lib/forge/interface/forgeChecksMonitor';
import type { ForgeIssueService } from '$lib/forge/interface/forgeIssueService';
import type { ForgeListingService } from '$lib/forge/interface/forgeListingService';
import type { ForgePrService } from '$lib/forge/interface/forgePrService';
import type { ForgeRepoService } from '$lib/forge/interface/forgeRepoService';

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

	// Host specific branch information.
	branch(name: string): ForgeBranch | undefined;

	// Web URL for a commit.
	commitUrl(id: string): string | undefined;
}

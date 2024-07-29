import { buildContextStore } from '$lib/utils/context';
import type { GitHostBranch } from './gitHostBranch';
import type { GitHostChecksMonitor } from './gitHostChecksMonitor';
import type { GitHostListingService } from './gitHostListingService';
import type { GitHostPrService } from './gitHostPrService';

export interface GitHost {
	// Lists PRs for the repo.
	listService(): GitHostListingService | undefined;

	// Detailed information about a specific PR.
	prService(baseBranch: string, upstreamName: string): GitHostPrService | undefined;

	// Results from CI check-runs.
	checksMonitor(branchName: string): GitHostChecksMonitor | undefined;

	// Host specific branch information.
	branch(name: string): GitHostBranch | undefined;

	// Web URL for a commit.
	commitUrl(id: string): string;
}

export const [getGitHost, createGitHostStore] = buildContextStore<GitHost | undefined>(
	'githubService'
);

import { buildContextStore } from '@gitbutler/shared/context';
import type { ForgeType } from '$lib/backend/forge';
import type { GitHostIssueService } from '$lib/gitHost/interface/gitHostIssueService';
import type { GitHostBranch } from './gitHostBranch';
import type { GitHostChecksMonitor } from './gitHostChecksMonitor';
import type { GitHostListingService } from './gitHostListingService';
import type { GitHostPrService } from './gitHostPrService';

export interface GitHost {
	readonly type: ForgeType;
	// Lists PRs for the repo.
	listService(): GitHostListingService | undefined;

	issueService(): GitHostIssueService | undefined;

	// Detailed information about a specific PR.
	prService(): GitHostPrService | undefined;

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

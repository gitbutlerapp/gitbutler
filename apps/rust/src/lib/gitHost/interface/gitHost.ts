import { buildContextStore } from '$lib/utils/context';
import type { GitHostChecksMonitor } from './gitHostChecksMonitor';
import type { GitHostListingService } from './gitHostListingService';
import type { GitHostPrService } from './gitHostPrService';

export interface GitHost {
	listService(): GitHostListingService;
	prService(baseBranch: string, upstreamName: string): GitHostPrService;
	checksMonitor(sourceBranch: string): GitHostChecksMonitor;
}

export const [getGitHost, createGitHostStore] = buildContextStore<GitHost | undefined>(
	'githubService'
);

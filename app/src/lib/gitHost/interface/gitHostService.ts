import { buildContextStore } from '$lib/utils/context';
import type { GitHostChecksMonitor } from './gitHostChecksMonitor';
import type { GitHostListingService } from './gitHostListingService';
import type { GitHostPrService } from './gitHostPrService';

export interface GitHostService {
	listService(): GitHostListingService;
	prService(baseBranch: string, upstreamName: string): GitHostPrService;
	checksMonitor(sourceBranch: string): GitHostChecksMonitor;
}

export const [getGitHostServiceStore, createGitHostServiceStore] = buildContextStore<
	GitHostService | undefined
>('githubService');

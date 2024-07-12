import { buildContextStore } from '$lib/utils/context';
import type { HostedGitChecksMonitor } from './hostedGitChecksMonitor';
import type { HostedGitListingService } from './hostedGitListingService';
import type { HostedGitPrService } from './hostedGitPrService';

export interface HostedGitService {
	listService(): HostedGitListingService;
	prService(baseBranch: string, upstreamName: string): HostedGitPrService;
	checksMonitor(sourceBranch: string): HostedGitChecksMonitor;
}

export const [getHostedGitServiceStore, createHostedGitServiceStore] = buildContextStore<
	HostedGitService | undefined
>('githubService');

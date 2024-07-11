import { buildContextStore } from '$lib/utils/context';
import type { HostedGitChecksMonitor } from './hostedGitChecksMonitor';
import type { HostedGitListingService } from './hostedGitListingService';
import type { HostedGitPrMonitor } from './hostedGitPrMonitor';
import type { HostedGitPrService } from './hostedGitPrService';

export interface HostedGitService {
	listService(): HostedGitListingService;
	prService(baseBranch: string, upstreamName: string): HostedGitPrService;
	prMonitor(prService: HostedGitPrService, prNumber: number): HostedGitPrMonitor;
	checksMonitor(sourceBranch: string): HostedGitChecksMonitor;
}

export const [getHostedGitServiceStore, createHostedGitServiceStore] = buildContextStore<
	HostedGitService | undefined
>('githubService');

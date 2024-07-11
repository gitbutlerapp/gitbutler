import { buildContextStore } from '$lib/utils/context';
import type { PullRequest } from './types';
import type { Writable } from 'svelte/store';

export const [getHostedGitListingServiceStore, createHostedGitListingServiceStore] =
	buildContextStore<HostedGitListingService | undefined>('gitListService');

export interface HostedGitListingService {
	prs: Writable<PullRequest[]>;
	reload(): Promise<void>;
}

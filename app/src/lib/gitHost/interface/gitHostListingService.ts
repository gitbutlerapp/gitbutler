import { buildContextStore } from '$lib/utils/context';
import type { PullRequest } from './types';
import type { Writable } from 'svelte/store';

export const [getGitHostListingService, createGitHostListingServiceStore] = buildContextStore<
	GitHostListingService | undefined
>('gitHostListingService');

export interface GitHostListingService {
	prs: Writable<PullRequest[]>;
	reload(): Promise<void>;
}

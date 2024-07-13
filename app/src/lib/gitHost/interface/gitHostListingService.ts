import { buildContextStore } from '$lib/utils/context';
import type { PullRequest } from './types';
import type { Writable } from 'svelte/store';

export const [getGitHostListingServiceStore, createGitHostListingServiceStore] = buildContextStore<
	GitHostListingService | undefined
>('gitListService');

export interface GitHostListingService {
	prs: Writable<PullRequest[]>;
	reload(): Promise<void>;
}

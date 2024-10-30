import { buildContextStore } from '@gitbutler/shared/context';
import type { PullRequest } from './types';
import type { Readable } from 'svelte/store';

export const [getGitHostListingService, createGitHostListingServiceStore] = buildContextStore<
	GitHostListingService | undefined
>('gitHostListingService');

export interface GitHostListingService {
	prs: Readable<PullRequest[]>;
	fetch(): Promise<PullRequest[]>;
	refresh(): Promise<void>;
}

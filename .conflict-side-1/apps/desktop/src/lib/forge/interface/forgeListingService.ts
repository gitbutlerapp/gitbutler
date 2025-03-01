import { buildContextStore } from '@gitbutler/shared/context';
import type { PullRequest } from './types';
import type { Readable } from 'svelte/store';

export const [getForgeListingService, createForgeListingServiceStore] = buildContextStore<
	ForgeListingService | undefined
>('forgeListingService');

export interface ForgeListingService {
	prs: Readable<PullRequest[]>;
	refresh(): Promise<void>;
}

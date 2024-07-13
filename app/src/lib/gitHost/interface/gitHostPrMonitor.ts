import { buildContextStore } from '$lib/utils/context';
import type { DetailedPullRequest } from './types';
import type { Readable } from 'svelte/store';

export const PR_MONITOR = Symbol('PullRequestMonitor');

export interface GitHostPrMonitor {
	pr: Readable<DetailedPullRequest | undefined>;
	loading?: Readable<boolean>;
	lastFetch?: Readable<Date | undefined>;
	refresh(): Promise<void>;
}

export const [getGitHostPrMonitorStore, createGitHostPrMonitorStore] = buildContextStore<
	GitHostPrMonitor | undefined
>('prMonitor');

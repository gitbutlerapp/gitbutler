import type { DetailedPullRequest } from '$lib/forge/interface/types';
import type { Readable } from 'svelte/store';

export interface ForgePrMonitor {
	pr: Readable<DetailedPullRequest | undefined>;
	loading?: Readable<boolean>;
	error: Readable<any>;
	refresh(): Promise<void>;
}

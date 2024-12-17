import { buildContextStore } from '@gitbutler/shared/context';
import type { DetailedPullRequest } from './types';
import type { Readable } from 'svelte/store';

export interface ForgePrMonitor {
	pr: Readable<DetailedPullRequest | undefined>;
	loading?: Readable<boolean>;
	mergedIncorrectly?: Readable<boolean>;
	error: Readable<any>;
	refresh(): Promise<void>;
}

export const [getForgePrMonitor, createForgePrMonitorStore] = buildContextStore<
	ForgePrMonitor | undefined
>('prMonitor');

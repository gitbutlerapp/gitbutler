import { buildContextStore } from '$lib/utils/context';
import type { ChecksStatus } from './types';
import type { Readable } from 'svelte/store';

export interface GitHostChecksMonitor {
	status: Readable<ChecksStatus | undefined | null>;
	loading?: Readable<boolean>;
	getLastStatus(): ChecksStatus | undefined | null;
	update(): Promise<void>;
	stop(): void;
}

export const [getGitHostChecksMonitorStore, createGitHostChecksMonitorStore] = buildContextStore<
	GitHostChecksMonitor | undefined
>('checksMonitor');

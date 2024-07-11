import { buildContextStore } from '$lib/utils/context';
import type { Readable } from 'svelte/store';

export interface CheckRun {
	startedAt: Date;
	completed: boolean;
	success: boolean;
	hasChecks: boolean;
	failed: number;
	queued: number;
	totalCount: number;
	skipped: number;
	finished: number;
}

export interface HostedGitChecksMonitor {
	result: Readable<CheckRun | undefined | null>;
	loading?: Readable<boolean>;
	refresh(): void;
}

export const [getHostedGitChecksMonitorStore, createHostedGitChecksMonitorStore] =
	buildContextStore<HostedGitChecksMonitor | undefined>('checksMonitor');

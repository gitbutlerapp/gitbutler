import { buildContextStore } from '@gitbutler/shared/context';
import type { ChecksStatus } from './types';
import type { Readable } from 'svelte/store';

export interface ForgeChecksMonitor {
	status: Readable<ChecksStatus | undefined | null>;
	loading?: Readable<boolean>;
	error: Readable<any>;
	getLastStatus(): ChecksStatus | undefined | null;
	update(): Promise<void>;
	stop(): void;
}

export const [getForgeChecksMonitor, createForgeChecksMonitorStore] = buildContextStore<
	ForgeChecksMonitor | undefined
>('checksMonitor');

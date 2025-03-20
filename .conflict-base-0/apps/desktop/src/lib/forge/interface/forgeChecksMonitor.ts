import type { ChecksStatus } from '$lib/forge/interface/types';
import type { Readable } from 'svelte/store';

export interface ForgeChecksMonitor {
	status: Readable<ChecksStatus | undefined | null>;
	loading?: Readable<boolean>;
	error: Readable<any>;
	getLastStatus(): ChecksStatus | undefined | null;
	update(): Promise<void>;
	stop(): void;
}

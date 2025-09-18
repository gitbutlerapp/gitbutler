import type { ChecksStatus } from '$lib/forge/interface/types';
import type { QueryOptions, ReactiveQuery } from '$lib/state/butlerModule';

export interface ChecksService {
	get(branch: string, options?: QueryOptions): ReactiveQuery<ChecksStatus | null>;
	fetch(branch: string, options?: QueryOptions): Promise<ChecksStatus | null>;
}

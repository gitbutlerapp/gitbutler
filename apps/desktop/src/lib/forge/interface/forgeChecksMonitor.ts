import type { ChecksStatus } from '$lib/forge/interface/types';
import type { AsyncResult, QueryOptions, ReactiveResult } from '$lib/state/butlerModule';

export interface ChecksService {
	get(branch: string, options?: QueryOptions): ReactiveResult<ChecksStatus | null>;
	fetch(branch: string, options?: QueryOptions): AsyncResult<ChecksStatus | null>;
}

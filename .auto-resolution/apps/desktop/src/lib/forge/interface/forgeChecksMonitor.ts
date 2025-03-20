import type { ChecksStatus } from './types';
import type { QueryOptions, ReactiveResult } from '$lib/state/butlerModule';

export interface ChecksService {
	get(branch: string, options?: QueryOptions): ReactiveResult<ChecksStatus | null>;
}

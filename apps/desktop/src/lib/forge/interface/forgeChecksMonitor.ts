import type { ChecksStatus } from '$lib/forge/interface/types';
import type { QueryOptions, ReactiveResult } from '$lib/state/butlerModule';

export interface ChecksService {
	get(stackId: string, branch: string, options?: QueryOptions): ReactiveResult<ChecksStatus | null>;
}

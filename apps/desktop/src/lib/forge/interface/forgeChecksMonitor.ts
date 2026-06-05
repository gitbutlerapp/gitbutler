import type { ChecksStatus } from "$lib/forge/interface/types";
import type { QueryExtensions, QueryOptions, ReactiveQuery } from "$lib/state/butlerModule";

export interface ChecksService {
	get(
		projectId: string,
		branch: string,
		options?: QueryOptions,
	): ReactiveQuery<ChecksStatus | null, QueryExtensions>;
	fetch(projectId: string, branch: string, options?: QueryOptions): Promise<ChecksStatus | null>;
}

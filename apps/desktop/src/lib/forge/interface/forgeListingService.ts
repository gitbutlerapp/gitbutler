import type { ReactiveResult } from '$lib/state/butlerModule';
import type { PullRequest } from './types';

export interface ForgeListingService {
	list(projectId: string): ReactiveResult<PullRequest[]>;
	getByBranch(projectId: string, branchName: string): ReactiveResult<PullRequest>;
	refresh(): Promise<void>;
}

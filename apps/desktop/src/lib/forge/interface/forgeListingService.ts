import type { ReactiveResult } from '$lib/state/butlerModule';
import type { PullRequest } from './types';
import type { Reactive } from '@gitbutler/shared/storeUtils';

export interface ForgeListingService {
	list(projectId: string, pollingInterval?: number): ReactiveResult<PullRequest[]>;
	getByBranch(projectId: string, branchName: string): ReactiveResult<PullRequest>;
	filterByBranch(projectId: string, branchName: string[]): Reactive<PullRequest[]>;
	refresh(projectId: string): Promise<void>;
}

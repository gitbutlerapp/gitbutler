import type { PullRequest } from '$lib/forge/interface/types';
import type { ReactiveResult } from '$lib/state/butlerModule';

export interface ForgeListingService {
	list(projectId: string, pollingInterval?: number): ReactiveResult<PullRequest[]>;
	getByBranch(projectId: string, branchName: string): ReactiveResult<PullRequest>;
	filterByBranch(projectId: string, branchName: string[]): ReactiveResult<PullRequest[]>;
	fetchByBranch(projectId: string, branchName: string[]): Promise<PullRequest[]>;
	refresh(projectId: string): Promise<void>;
}

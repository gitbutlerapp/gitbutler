import type { PullRequest } from '$lib/forge/interface/types';
import type { QueryExtensions, ReactiveQuery } from '$lib/state/butlerModule';

export interface ForgeListingService {
	list(projectId: string, pollingInterval?: number): ReactiveQuery<PullRequest[], QueryExtensions>;
	getByBranch(projectId: string, branchName: string): ReactiveQuery<PullRequest | undefined>;
	filterByBranch(
		projectId: string,
		branchName: string[]
	): ReactiveQuery<PullRequest[], QueryExtensions>;
	fetchByBranch(projectId: string, branchName: string[]): Promise<PullRequest[]>;
	refresh(projectId: string): Promise<void>;
}

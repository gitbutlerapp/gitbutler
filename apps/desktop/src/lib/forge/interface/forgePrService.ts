import type { ReactiveResult } from '$lib/state/butlerModule';
import type { CreatePullRequestArgs, DetailedPullRequest, MergeMethod, PullRequest } from './types';
import type { MergeResult, UpdateResult } from '../github/types';
import type { SubscriptionOptions } from '@reduxjs/toolkit/query';
import type { Writable } from 'svelte/store';

export interface ForgePrService {
	loading: Writable<boolean>;
	get(prNumber: number, options?: SubscriptionOptions): ReactiveResult<DetailedPullRequest>;
	fetch(prNumber: number): Promise<DetailedPullRequest>;
	createPr({
		title,
		body,
		draft,
		baseBranchName,
		upstreamName
	}: CreatePullRequestArgs): Promise<PullRequest>;
	merge(method: MergeMethod, prNumber: number): Promise<MergeResult>;
	reopen(prNumber: number): Promise<UpdateResult>;
	update(
		prNumber: number,
		details: { description?: string; state?: 'open' | 'closed'; targetBase?: string }
	): Promise<UpdateResult>;
}

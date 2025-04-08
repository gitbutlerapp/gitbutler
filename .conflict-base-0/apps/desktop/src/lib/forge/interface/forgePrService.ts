import type {
	CreatePullRequestArgs,
	DetailedPullRequest,
	MergeMethod,
	PullRequest
} from '$lib/forge/interface/types';
import type { AsyncResult, ReactiveResult } from '$lib/state/butlerModule';
import type { StartQueryActionCreatorOptions } from '@reduxjs/toolkit/query';
import type { Writable } from 'svelte/store';

export interface ForgePrService {
	loading: Writable<boolean>;
	get(
		prNumber: number,
		options?: StartQueryActionCreatorOptions
	): ReactiveResult<DetailedPullRequest>;
	fetch(
		prNumber: number,
		options?: StartQueryActionCreatorOptions
	): AsyncResult<DetailedPullRequest | undefined>;
	createPr({
		title,
		body,
		draft,
		baseBranchName,
		upstreamName
	}: CreatePullRequestArgs): Promise<PullRequest>;
	merge(method: MergeMethod, prNumber: number): Promise<void>;
	reopen(prNumber: number): Promise<void>;
	update(
		prNumber: number,
		details: { description?: string; state?: 'open' | 'closed'; targetBase?: string }
	): Promise<void>;
}

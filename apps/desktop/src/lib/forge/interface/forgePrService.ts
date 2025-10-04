import type {
	CreatePullRequestArgs,
	DetailedPullRequest,
	MergeMethod,
	PullRequest
} from '$lib/forge/interface/types';
import type { ReactiveQuery } from '$lib/state/butlerModule';
import type { StartQueryActionCreatorOptions } from '@reduxjs/toolkit/query';
import type { Writable } from 'svelte/store';

export type ReviewUnitInfo = {
	name: string;
	abbr: string;
	symbol: string;
};

export interface ForgePrService {
	readonly unit: ReviewUnitInfo;
	loading: Writable<boolean>;
	get(
		prNumber: number,
		options?: StartQueryActionCreatorOptions
	): ReactiveQuery<DetailedPullRequest>;
	fetch(
		prNumber: number,
		options?: StartQueryActionCreatorOptions
	): Promise<DetailedPullRequest | undefined>;
	createPr({
		title,
		body,
		draft,
		baseBranchName,
		upstreamName
	}: CreatePullRequestArgs): Promise<PullRequest>;
	merge(method: MergeMethod, prNumber: number, bypassRules?: boolean): Promise<void>;
	reopen(prNumber: number): Promise<void>;
	update(
		prNumber: number,
		details: { description?: string; state?: 'open' | 'closed'; targetBase?: string }
	): Promise<void>;
}

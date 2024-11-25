import { buildContextStore } from '@gitbutler/shared/context';
import type { ForgePrMonitor } from './forgePrMonitor';
import type { CreatePullRequestArgs, DetailedPullRequest, MergeMethod, PullRequest } from './types';
import type { Writable } from 'svelte/store';

export const [getForgePrService, createForgePrServiceStore] = buildContextStore<
	ForgePrService | undefined
>('forgePrService');

export interface ForgePrService {
	loading: Writable<boolean>;
	get(prNumber: number): Promise<DetailedPullRequest>;
	createPr({
		title,
		body,
		draft,
		baseBranchName,
		upstreamName
	}: CreatePullRequestArgs): Promise<PullRequest>;
	merge(method: MergeMethod, prNumber: number): Promise<void>;
	updateBase(prNumber: number, targetBase: string): Promise<void>;
	reopen(prNumber: number): Promise<void>;
	prMonitor(prNumber: number): ForgePrMonitor;
	update(
		prNumber: number,
		details: { description?: string; state?: 'open' | 'closed'; targetBase?: string }
	): Promise<void>;
}

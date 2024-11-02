import { buildContextStore } from '@gitbutler/shared/context';
import type { ForgePrMonitor } from './forgePrMonitor';
import type {
	CreatePullRequestArgs,
	DetailedPullRequest,
	PullRequestId,
	MergeMethod,
	PullRequest
} from './types';
import type { Writable } from 'svelte/store';

export const [getForgePrService, createForgePrServiceStore] = buildContextStore<
	ForgePrService | undefined
>('forgePrService');

export interface ForgePrService {
	loading: Writable<boolean>;
	get(id: PullRequestId): Promise<DetailedPullRequest>;
	createPr({
		title,
		body,
		draft,
		baseBranchName,
		upstreamName
	}: CreatePullRequestArgs): Promise<PullRequest>;
	merge(method: MergeMethod, id: PullRequestId): Promise<void>;
	reopen(id: PullRequestId): Promise<void>;
	prMonitor(id: PullRequestId): ForgePrMonitor;
}

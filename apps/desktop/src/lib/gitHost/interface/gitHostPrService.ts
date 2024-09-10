import { buildContextStore } from '$lib/utils/context';
import type { GitHostPrMonitor } from './gitHostPrMonitor';
import type { DetailedPullRequest, MergeMethod, PullRequest } from './types';
import type { Writable } from 'svelte/store';

export const [getGitHostPrService, createGitHostPrServiceStore] = buildContextStore<
	GitHostPrService | undefined
>('gitBranchService');

export interface GitHostPrService {
	loading: Writable<boolean>;
	get(prNumber: number): Promise<DetailedPullRequest>;
	createPr(
		title: string,
		body: string,
		draft: boolean,
		baseBranchName: string,
		upstreamName: string
	): Promise<PullRequest>;
	fetchPrTemplate(path?: string): Promise<string | undefined>;
	merge(method: MergeMethod, prNumber: number): Promise<void>;
	prMonitor(prNumber: number): GitHostPrMonitor;
}

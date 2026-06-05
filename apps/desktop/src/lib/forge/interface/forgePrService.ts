import type { CreatePullRequestArgs, MergeMethod, PullRequest } from "$lib/forge/interface/types";
import type { ReactiveQuery } from "$lib/state/butlerModule";
import type { ReviewMergeStatus } from "@gitbutler/but-sdk";
import type { StartQueryActionCreatorOptions } from "@reduxjs/toolkit/query";
import type { Writable } from "svelte/store";

export type ReviewUnitInfo = {
	name: string;
	abbr: string;
	symbol: string;
};

export interface ForgePrService {
	readonly unit: ReviewUnitInfo;
	loading: Writable<boolean>;
	get(
		projectId: string,
		prNumber: number,
		options?: StartQueryActionCreatorOptions,
	): ReactiveQuery<PullRequest>;
	fetch(
		projectId: string,
		prNumber: number,
		options?: StartQueryActionCreatorOptions,
	): Promise<PullRequest | undefined>;
	createPr(
		projectId: string,
		{ title, body, draft, baseBranchName, upstreamName }: CreatePullRequestArgs,
	): Promise<PullRequest>;
	/**
	 * Forces the forge to compute merge state server-side. Subscribe
	 * only where the merge hint or comment count is rendered.
	 */
	getMergeStatus(projectId: string, prNumber: number): ReactiveQuery<ReviewMergeStatus>;
	/** `null` on forges where fork detection is not URL-based. */
	getBaseRepoUrl(projectId: string, prNumber: number): ReactiveQuery<string | null>;
	merge(projectId: string, method: MergeMethod, prNumber: number): Promise<void>;
	reopen(projectId: string, prNumber: number): Promise<void>;
	update(
		projectId: string,
		prNumber: number,
		details: { description?: string; state?: "open" | "closed"; targetBase?: string },
	): Promise<void>;
	setDraft(projectId: string, prNumber: number, draft: boolean): Promise<void>;
}

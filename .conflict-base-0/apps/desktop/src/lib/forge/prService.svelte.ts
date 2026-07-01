import {
	MergeMethod,
	mapForgeReviewToPullRequest,
	type CreatePullRequestArgs,
	type ForgeReview,
	type PullRequest,
} from "$lib/forge/interface/types";
import { invalidatesItem, invalidatesList, providesItem, ReduxTag } from "$lib/state/tags";
import { sleep } from "$lib/utils/sleep";
import { InjectionToken } from "@gitbutler/core/context";
import { writable } from "svelte/store";
import type { BackendApi } from "$lib/state/backendApi";
import type { QueryOptions } from "$lib/state/butlerModule";
import type { PostHogWrapper } from "$lib/telemetry/posthog";
import type { ReviewMergeStatus } from "@gitbutler/but-sdk";
import type { StartQueryActionCreatorOptions } from "@reduxjs/toolkit/query";

export const PR_SERVICE = new InjectionToken<PrService>("PrService");

const pendingReviewFetches = new Map<string, Set<Promise<unknown>>>();

function trackReviewFetch(projectId: string, pending: Promise<unknown>) {
	let pendingForProject = pendingReviewFetches.get(projectId);
	if (!pendingForProject) {
		pendingForProject = new Set();
		pendingReviewFetches.set(projectId, pendingForProject);
	}
	pendingForProject.add(pending);
	pending.finally(() => {
		pendingForProject.delete(pending);
		if (pendingForProject.size === 0) {
			pendingReviewFetches.delete(projectId);
		}
	});
}

export class PrService {
	loading = writable(false);
	private backendApi: ReturnType<typeof injectBackendEndpoints>;

	constructor(
		backendApi: BackendApi,
		private posthog?: PostHogWrapper,
	) {
		this.backendApi = injectBackendEndpoints(backendApi);
	}

	async createPr(
		projectId: string,
		{
			title,
			body,
			draft,
			baseBranchName,
			upstreamName,
			posthogLabel,
		}: CreatePullRequestArgs & { posthogLabel?: string },
	): Promise<PullRequest> {
		this.loading.set(true);
		const request = async () => {
			const review = await this.backendApi.endpoints.publishReview.mutate({
				projectId,
				params: {
					title,
					body,
					sourceBranch: upstreamName,
					targetBranch: baseBranchName,
					draft,
				},
			});
			return mapForgeReviewToPullRequest(review);
		};

		let attempts = 0;
		let lastError: any;

		// Use retries since request can fail right after branch push.
		while (attempts < 4) {
			try {
				const pr = await request();
				if (posthogLabel) this.posthog?.capture(`${posthogLabel} Successful`);
				return pr;
			} catch (err: any) {
				lastError = err;
				attempts++;
				await sleep(500);
			} finally {
				this.loading.set(false);
			}
		}
		if (posthogLabel) this.posthog?.capture(`${posthogLabel} Failure`);
		throw lastError;
	}

	async fetch(projectId: string, number: number, options?: QueryOptions) {
		const review = await this.backendApi.endpoints.getReview.fetch(
			{ projectId, reviewId: number },
			options,
		);
		return review ? mapForgeReviewToPullRequest(review) : undefined;
	}

	async waitForRefreshes(projectId: string): Promise<void> {
		let pending = pendingReviewFetches.get(projectId);
		while (pending && pending.size > 0) {
			await Promise.allSettled([...pending]);
			pending = pendingReviewFetches.get(projectId);
		}
	}

	get(projectId: string, number: number, options?: StartQueryActionCreatorOptions) {
		return this.backendApi.endpoints.getReview.useQuery(
			{ projectId, reviewId: number },
			{
				...options,
				transform: (result) => mapForgeReviewToPullRequest(result),
			},
		);
	}

	getMergeStatus(projectId: string, number: number) {
		return this.backendApi.endpoints.getMergeStatus.useQuery({
			projectId,
			reviewId: number,
		});
	}

	getBaseRepoUrl(projectId: string, number: number) {
		return this.backendApi.endpoints.getBaseRepoUrl.useQuery({
			projectId,
			reviewId: number,
		});
	}

	async merge(projectId: string, method: MergeMethod, number: number) {
		await this.backendApi.endpoints.mergeReview.mutate({
			projectId,
			reviewId: number,
			mergeMethod: method,
		});
	}

	async reopen(projectId: string, number: number) {
		await this.backendApi.endpoints.updateReview.mutate({
			projectId,
			reviewId: number,
			title: null,
			body: null,
			state: "open",
			targetBase: null,
		});
	}

	async update(
		projectId: string,
		number: number,
		update: {
			title?: string;
			description?: string;
			state?: "open" | "closed";
			targetBase?: string;
		},
	) {
		await this.backendApi.endpoints.updateReview.mutate({
			projectId,
			reviewId: number,
			title: update.title ?? null,
			body: update.description ?? null,
			state: update.state ?? null,
			targetBase: update.targetBase ?? null,
		});
	}

	async setDraft(projectId: string, reviewId: number, draft: boolean) {
		await this.backendApi.endpoints.setDraft.mutate({ projectId, reviewId, draft });
	}
}

function injectBackendEndpoints(api: BackendApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			setAutoMerge: build.mutation<void, { projectId: string; reviewId: number; enable: boolean }>({
				extraOptions: { command: "set_review_auto_merge" },
				query: (args) => args,
				invalidatesTags: (_res, _err, { reviewId }) => [
					invalidatesItem(ReduxTag.PullRequests, reviewId),
				],
			}),
			setDraft: build.mutation<void, { projectId: string; reviewId: number; draft: boolean }>({
				extraOptions: { command: "set_review_draftiness" },
				query: (args) => args,
				invalidatesTags: (_res, _err, { reviewId }) => [
					invalidatesItem(ReduxTag.PullRequests, reviewId),
				],
			}),
			mergeReview: build.mutation<
				void,
				{ projectId: string; reviewId: number; mergeMethod: MergeMethod }
			>({
				extraOptions: { command: "merge_review" },
				query: (args) => args,
				invalidatesTags: (_res, _err, { reviewId }) => [
					invalidatesItem(ReduxTag.PullRequests, reviewId),
				],
			}),
			publishReview: build.mutation<
				ForgeReview,
				{
					projectId: string;
					params: {
						title: string;
						body: string;
						sourceBranch: string;
						targetBranch: string;
						draft: boolean;
					};
				}
			>({
				extraOptions: { command: "publish_review" },
				query: (args) => args,
				invalidatesTags: [invalidatesList(ReduxTag.PullRequests)],
			}),
			getReview: build.query<ForgeReview, { projectId: string; reviewId: number }>({
				extraOptions: { command: "get_review" },
				query: (args) => args,
				providesTags: (_result, _error, args) => providesItem(ReduxTag.PullRequests, args.reviewId),
				onQueryStarted: (args, { queryFulfilled }) => {
					const pending = queryFulfilled.catch(() => undefined);
					trackReviewFetch(args.projectId, pending);
				},
			}),
			getMergeStatus: build.query<ReviewMergeStatus, { projectId: string; reviewId: number }>({
				extraOptions: { command: "get_review_merge_status" },
				query: (args) => args,
				providesTags: (_result, _error, args) => providesItem(ReduxTag.PullRequests, args.reviewId),
			}),
			getBaseRepoUrl: build.query<string | null, { projectId: string; reviewId: number }>({
				extraOptions: { command: "get_review_base_repo_url" },
				query: (args) => args,
				providesTags: (_result, _error, args) => providesItem(ReduxTag.PullRequests, args.reviewId),
			}),
			updateReview: build.mutation<
				void,
				{
					projectId: string;
					reviewId: number;
					title: string | null;
					body: string | null;
					state: "open" | "closed" | null;
					targetBase: string | null;
				}
			>({
				extraOptions: { command: "update_review" },
				query: (args) => args,
				invalidatesTags: (_res, _err, { reviewId }) => [
					invalidatesItem(ReduxTag.PullRequests, reviewId),
				],
			}),
		}),
	});
}

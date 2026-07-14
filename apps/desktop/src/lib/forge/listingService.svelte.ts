import {
	mapForgeReviewToPullRequest,
	type ForgeReview,
	type PullRequest,
} from "$lib/forge/interface/types";
import { createSelectByIds } from "$lib/state/customSelectors";
import { invalidatesList, providesList, ReduxTag } from "$lib/state/tags";
import { InjectionToken } from "@gitbutler/core/context";
import { isDefined } from "@gitbutler/ui/utils/typeguards";
import { createEntityAdapter, type EntityState } from "@reduxjs/toolkit";
import type { BackendApi } from "$lib/state/backendApi";
import type { AppDispatch } from "$lib/state/clientState.svelte";
import type { CacheConfig } from "@gitbutler/but-sdk";

export const LISTING_SERVICE = new InjectionToken<ListingService>("ListingService");

// Serve a brief cache window so repeated/cold reads of the review list are
// snappy instead of waiting on a forge round-trip every time. The periodic
// poll and the Sync button (via `refresh()`, which forces `noCache`) still
// pull live data past this window.
const REVIEWS_CACHE: CacheConfig = { cacheWithFallback: { max_age_seconds: 300 } };

export class ListingService {
	private backendApi: ReturnType<typeof injectBackendEndpoints>;

	constructor(
		backendApi: BackendApi,
		private readonly dispatch: AppDispatch,
	) {
		this.backendApi = injectBackendEndpoints(backendApi);
	}

	list(projectId: string, pollingInterval?: number) {
		return this.backendApi.endpoints.listPrs.useQuery(projectId, {
			transform: (result) => prSelectors.selectAll(result),
			subscriptionOptions: { pollingInterval },
		});
	}

	getByBranch(projectId: string, branchName: string) {
		return this.backendApi.endpoints.listPrs.useQuery(projectId, {
			transform: (result) => prSelectors.selectById(result, branchName),
		});
	}

	filterByBranch(projectId: string, branchName: string[]) {
		return this.backendApi.endpoints.listPrs.useQueryState(projectId, {
			transform: (result) => prSelectors.selectByIds(result, branchName),
		});
	}

	async fetchByBranch(projectId: string, branchNames: string[]) {
		const result = await this.backendApi.endpoints.listPrs.fetch(projectId);
		if (!result) return [];
		return branchNames.map((branch) => prSelectors.selectById(result, branch)).filter(isDefined);
	}

	async refresh(projectId: string): Promise<void> {
		// Force a live fetch so the DB cache is refreshed, then invalidate the
		// cached listing so its subscribers re-read the just-updated data.
		await this.backendApi.endpoints.listPrsLive.fetch(projectId);
		this.dispatch(this.backendApi.util.invalidateTags([invalidatesList(ReduxTag.PullRequests)]));
	}
}

function injectBackendEndpoints(api: BackendApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			listPrs: build.query<EntityState<PullRequest, string>, string>({
				extraOptions: {
					command: "list_reviews",
				},
				query: (projectId) => ({ projectId, cacheConfig: REVIEWS_CACHE }),
				transformResponse: (response: ForgeReview[]) => {
					const prs = response.map((pr) => mapForgeReviewToPullRequest(pr));
					return prAdapter.addMany(prAdapter.getInitialState(), prs);
				},
				async onQueryStarted(_projectId, { dispatch, queryFulfilled }) {
					try {
						// `list_reviews` updates the backend forge cache. Workspace PR
						// associations are projected from that cache, so rebuild the
						// workspace view only after the cache-writing request succeeds.
						await queryFulfilled;
						dispatch(
							api.util.invalidateTags([
								invalidatesList(ReduxTag.Stacks),
								invalidatesList(ReduxTag.StackDetails),
							]),
						);
					} catch {
						// Keep the last cache-derived workspace view when the forge is
						// unavailable. The query exposes the listing error separately.
					}
				},
				providesTags: [providesList(ReduxTag.PullRequests)],
			}),
			// One-shot live fetch used by `refresh()` to repopulate the DB
			// cache; subscribers read through `listPrs` after invalidation.
			listPrsLive: build.query<ForgeReview[], string>({
				extraOptions: {
					command: "list_reviews",
				},
				query: (projectId) => ({ projectId, cacheConfig: "noCache" }),
			}),
		}),
	});
}

const prAdapter = createEntityAdapter<PullRequest, string>({
	selectId: (pr) => pr.sourceBranch,
});

const prSelectors = { ...prAdapter.getSelectors(), selectByIds: createSelectByIds<PullRequest>() };

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

export const LISTING_SERVICE = new InjectionToken<ListingService>("ListingService");

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

	async refresh(_projectId: string): Promise<void> {
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
				query: (projectId) => ({ projectId }),
				transformResponse: (response: ForgeReview[]) => {
					const prs = response.map((pr) => mapForgeReviewToPullRequest(pr));
					return prAdapter.addMany(prAdapter.getInitialState(), prs);
				},
				providesTags: [providesList(ReduxTag.PullRequests)],
			}),
		}),
	});
}

const prAdapter = createEntityAdapter<PullRequest, string>({
	selectId: (pr) => pr.sourceBranch,
});

const prSelectors = { ...prAdapter.getSelectors(), selectByIds: createSelectByIds<PullRequest>() };

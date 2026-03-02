import { gitea } from "$lib/forge/gitea/giteaClient.svelte";
import { prToInstance, splitGiteaProjectId } from "$lib/forge/gitea/types";
import { createSelectByIds } from "$lib/state/customSelectors";
import { combineResults } from "$lib/state/helpers";
import { invalidatesList, providesList, ReduxTag } from "$lib/state/tags";
import { toSerializable } from "@gitbutler/shared/network/types";
import { isDefined } from "@gitbutler/ui/utils/typeguards";
import {
	createEntityAdapter,
	type EntityState,
	type ThunkDispatch,
	type UnknownAction,
} from "@reduxjs/toolkit";
import type { ForgeListingService } from "$lib/forge/interface/forgeListingService";
import type { PullRequest } from "$lib/forge/interface/types";
import type { GiteaApi } from "$lib/state/clientState.svelte";
import type { BaseQueryApi } from "@reduxjs/toolkit/query";

export class GiteaListingService implements ForgeListingService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(
		giteaApi: GiteaApi,
		private readonly dispatch: ThunkDispatch<any, any, UnknownAction>,
	) {
		this.api = injectEndpoints(giteaApi);
	}

	list(projectId: string, pollingInterval?: number) {
		return this.api.endpoints.listPrs.useQuery(projectId, {
			transform: (result) => prSelectors.selectAll(result),
			subscriptionOptions: { pollingInterval },
		});
	}

	getByBranch(projectId: string, branchName: string) {
		return this.api.endpoints.listPrs.useQuery(projectId, {
			transform: (result) => prSelectors.selectById(result, branchName),
		});
	}

	filterByBranch(projectId: string, branchNames: string[]) {
		return this.api.endpoints.listPrs.useQueryState(projectId, {
			transform: (result) => prSelectors.selectByIds(result, branchNames),
		});
	}

	async fetchByBranch(projectId: string, branchNames: string[]) {
		const results = await Promise.all(
			branchNames.map((branch) =>
				this.api.endpoints.listPrsByBranch.fetch({ projectId, branchName: branch }),
			),
		);
		const combined = combineResults(...results);
		return combined.data?.filter(isDefined) ?? [];
	}

	async refresh(_projectId: string): Promise<void> {
		this.dispatch(this.api.util.invalidateTags([invalidatesList(ReduxTag.GiteaPullRequests)]));
	}
}

async function getAllPrs(query: BaseQueryApi, _projectId: string) {
	const { api, upstreamProjectId, forkProjectId } = gitea(query.extra);
	const { owner, repo } = splitGiteaProjectId(upstreamProjectId);
	const { owner: forkOwner, repo: forkRepo } = splitGiteaProjectId(forkProjectId);

	const isSameRepo = owner === forkOwner && repo === forkRepo;

	const [upstreamPrs, forkPrs] = await Promise.all([
		api.repos.repoListPullRequests(owner, repo, { state: "open", limit: 50 }),
		!isSameRepo
			? api.repos.repoListPullRequests(forkOwner, forkRepo, { state: "open", limit: 50 })
			: Promise.resolve({ data: [] as any[] }),
	]);

	return [...(upstreamPrs.data ?? []), ...(forkPrs.data ?? [])];
}

function injectEndpoints(api: GiteaApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			listPrs: build.query<EntityState<PullRequest, string>, string>({
				queryFn: async (projectId, query) => {
					try {
						const prs = await getAllPrs(query, projectId);
						return {
							data: prAdapter.addMany(
								prAdapter.getInitialState(),
								prs.map((data) => prToInstance({ data })),
							),
						};
					} catch (e: unknown) {
						return { error: toSerializable(e) };
					}
				},
				providesTags: [providesList(ReduxTag.GiteaPullRequests)],
			}),
			listPrsByBranch: build.query<
				PullRequest | null,
				{ projectId: string; branchName: string }
			>({
				queryFn: async ({ branchName, projectId }, query) => {
					try {
						const allPrs = await getAllPrs(query, projectId);
						const prsOnBranch = allPrs.filter((pr) => pr.head?.ref === branchName);

						if (prsOnBranch.length === 0) {
							return { data: null };
						}

						if (prsOnBranch.length > 1) {
							return {
								error: toSerializable(
									new Error(
										`Multiple pull requests found for branch ${branchName}`,
									),
								),
							};
						}

						return { data: prToInstance({ data: prsOnBranch[0]! }) };
					} catch (e: unknown) {
						return { error: toSerializable(e) };
					}
				},
			}),
		}),
	});
}

const prAdapter = createEntityAdapter<PullRequest, string>({
	selectId: (pr) => pr.sourceBranch,
});

const prSelectors = {
	...prAdapter.getSelectors(),
	selectByIds: createSelectByIds<PullRequest>(),
};

import { gitlab } from '$lib/forge/gitlab/gitlabClient.svelte';
import { mrToInstance } from '$lib/forge/gitlab/types';
import { createSelectByIds } from '$lib/state/customSelectors';
import { invalidatesList, providesList, ReduxTag } from '$lib/state/tags';
import { toSerializable } from '@gitbutler/shared/network/types';
import { isDefined } from '@gitbutler/ui/utils/typeguards';
import {
	createEntityAdapter,
	type EntityState,
	type ThunkDispatch,
	type UnknownAction
} from '@reduxjs/toolkit';
import type { ForgeListingService } from '$lib/forge/interface/forgeListingService';
import type { PullRequest } from '$lib/forge/interface/types';
import type { GitLabApi } from '$lib/state/clientState.svelte';

export class GitLabListingService implements ForgeListingService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(
		gitLabApi: GitLabApi,
		private readonly dispatch: ThunkDispatch<any, any, UnknownAction>
	) {
		this.api = injectEndpoints(gitLabApi);
	}

	list(projectId: string, pollingInterval?: number) {
		return this.api.endpoints.listPrs.useQuery(projectId, {
			transform: (result) => prSelectors.selectAll(result),
			subscriptionOptions: { pollingInterval }
		});
	}

	getByBranch(projectId: string, branchName: string) {
		return this.api.endpoints.listPrs.useQuery(projectId, {
			transform: (result) => prSelectors.selectById(result, branchName)
		});
	}

	filterByBranch(projectId: string, branchName: string[]) {
		return this.api.endpoints.listPrs.useQueryState(projectId, {
			transform: (result) => prSelectors.selectByIds(result, branchName)
		});
	}

	async fetchByBranch(projectId: string, branchNames: string[]) {
		const results = await Promise.all(
			branchNames.map((branch) =>
				this.api.endpoints.listPrsByBranch.fetch({ projectId, branchName: branch })
			)
		);
		return results.filter(isDefined) ?? [];
	}

	async refresh(_projectId: string): Promise<void> {
		this.dispatch(this.api.util.invalidateTags([invalidatesList(ReduxTag.PullRequests)]));
	}
}

function injectEndpoints(api: GitLabApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			listPrs: build.query<EntityState<PullRequest, string>, string>({
				queryFn: async (_, query) => {
					try {
						const { api, upstreamProjectId, forkProjectId } = gitlab(query.extra);
						const upstreamMrs = await api.MergeRequests.all({
							projectId: upstreamProjectId,
							state: 'opened'
						});
						const forkMrs = await api.MergeRequests.all({
							projectId: forkProjectId,
							state: 'opened'
						});

						return {
							data: prAdapter.addMany(
								prAdapter.getInitialState(),
								[...upstreamMrs, ...forkMrs].map((mr) => mrToInstance(mr))
							)
						};
					} catch (e: unknown) {
						return { error: toSerializable(e) };
					}
				},
				providesTags: [providesList(ReduxTag.PullRequests)]
			}),
			listPrsByBranch: build.query<PullRequest | null, { projectId: string; branchName: string }>({
				queryFn: async ({ branchName }, query) => {
					try {
						const { api, upstreamProjectId, forkProjectId } = gitlab(query.extra);
						const upstreamMrs = await api.MergeRequests.all({
							projectId: upstreamProjectId,
							sourceBranch: branchName,
							state: 'opened'
						});
						const forkMrs = await api.MergeRequests.all({
							projectId: forkProjectId,
							sourceBranch: branchName,
							state: 'opened'
						});

						const allMrs = [...upstreamMrs, ...forkMrs];
						if (allMrs.length === 0) {
							return { data: null };
						}

						if (allMrs.length > 1) {
							return { error: new Error(`Multiple merge requests found for branch ${branchName}`) };
						}

						const mrData = allMrs[0]!;
						const mr = mrToInstance(mrData);

						return {
							data: mr
						};
					} catch (e: unknown) {
						return { error: toSerializable(e) };
					}
				}
			})
		})
	});
}

const prAdapter = createEntityAdapter<PullRequest, string>({
	selectId: (pr) => pr.sourceBranch
});

const prSelectors = { ...prAdapter.getSelectors(), selectByIds: createSelectByIds<PullRequest>() };

// if (err.message.includes('you appear to have the correct authorization credentials')) {
// 	this.disabled = true;
// }

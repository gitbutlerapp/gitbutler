import { createSelectByIds } from '$lib/state/customSelectors';
import { combineResults } from '$lib/state/helpers';
import { providesList, ReduxTag } from '$lib/state/tags';
import { reactive } from '@gitbutler/shared/storeUtils';
import { isDefined } from '@gitbutler/ui/utils/typeguards';
import { createEntityAdapter, type EntityState } from '@reduxjs/toolkit';
import type { ForgeListingService } from '$lib/forge/interface/forgeListingService';
import type { PullRequest } from '$lib/forge/interface/types';
import type { ProjectMetrics } from '$lib/metrics/projectMetrics';
import type { GiteaApi } from '$lib/state/clientState.svelte';
import { gitea } from '$lib/forge/gitea/giteaClient.svelte';
import { prToInstance, splitGiteaProjectId } from '$lib/forge/gitea/types';
import type { BaseQueryApi } from '@reduxjs/toolkit/query';

export class GiteaListingService implements ForgeListingService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(
		giteaApi: GiteaApi,
		private projectMetrics?: ProjectMetrics
	) {
		this.api = injectEndpoints(giteaApi);
	}

	list(projectId: string, pollingInterval?: number) {
		const result = $derived(
			this.api.endpoints.listPrs.useQuery(projectId, {
				transform: (result) => prSelectors.selectAll(result),
				subscriptionOptions: { pollingInterval }
			})
		);
		$effect(() => {
			const items = result.current.data;
			if (items) {
				this.projectMetrics?.setMetric(projectId, 'pr_count', items.length);
			}
		});
		return reactive(() => result.current);
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
		const combined = combineResults(...results);

		return combined.data?.filter(isDefined) ?? [];
	}

	async refresh(projectId: string): Promise<void> {
		await this.api.endpoints.listPrs.fetch(projectId, { forceRefetch: true });
	}
}

async function getAllPrs(query: BaseQueryApi) {
	const { api, upstreamProjectId, forkProjectId } = gitea(query.extra);
	const { owner, repo } = splitGiteaProjectId(upstreamProjectId);
	const { owner: forkOwner, repo: forkRepo } = splitGiteaProjectId(forkProjectId);
	const upstreamPrs = await api.repos.repoListPullRequests(owner, repo, {
		state: 'open'
	});
	const forkPrs = await api.repos.repoListPullRequests(forkOwner, forkRepo, {
		state: 'open'
	});
	return [...upstreamPrs.data, ...forkPrs.data];
}

function injectEndpoints(api: GiteaApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			listPrs: build.query<EntityState<PullRequest, string>, string>({
				queryFn: async (_, query) => {
					var prs = await getAllPrs(query);
					return {
						data: prAdapter.addMany(
							prAdapter.getInitialState(),
							prs.map((data) => prToInstance({ data }))
						)
					};
				},
				providesTags: [providesList(ReduxTag.GiteaPullRequests)]
			}),
			listPrsByBranch: build.query<PullRequest | null, { projectId: string; branchName: string }>({
				queryFn: async ({ branchName }, query) => {
					var allPrs = await getAllPrs(query);

					var allPrsOnBranch = allPrs.filter((e) => e.base?.ref == branchName);

					if (allPrsOnBranch.length === 0) {
						return { data: null };
					}

					if (allPrsOnBranch.length > 1) {
						return { error: new Error(`Multiple merge requests found for branch ${branchName}`) };
					}

					const data = allPrsOnBranch[0]!;

					return {
						data: prToInstance({ data })
					};
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

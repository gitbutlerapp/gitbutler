import { gitea } from '$lib/forge/gitea/giteaClient.svelte';
import { giteaPrToInstance } from '$lib/forge/gitea/types';
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
import type { GiteaApi } from '$lib/state/clientState.svelte';

export class GiteaListingService implements ForgeListingService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(
		giteaApi: GiteaApi,
		private readonly dispatch: ThunkDispatch<any, any, UnknownAction>
	) {
		this.api = injectEndpoints(giteaApi);
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

function injectEndpoints(api: GiteaApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			listPrs: build.query<EntityState<PullRequest, string>, string>({
				queryFn: async (_, query) => {
					try {
						const { client, owner, repo } = gitea(query.extra);
						const prs = await client.listOpenPulls(owner, repo);

						return {
							data: prAdapter.addMany(
								prAdapter.getInitialState(),
								prs.map((pr) => giteaPrToInstance(pr))
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
						const { client, owner, repo } = gitea(query.extra);
						const prs = await client.listPullsByBranch(owner, repo, branchName);

						if (prs.length === 0) {
							return { data: null };
						}

						if (prs.length > 1) {
							return { error: new Error(`Multiple pull requests found for branch ${branchName}`) };
						}

						return { data: giteaPrToInstance(prs[0]!) };
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

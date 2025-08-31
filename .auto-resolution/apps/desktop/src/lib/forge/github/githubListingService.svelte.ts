import { ghQuery } from '$lib/forge/github/ghQuery';
import { ghResponseToInstance } from '$lib/forge/github/types';
import { createSelectByIds } from '$lib/state/customSelectors';
import { providesList, ReduxTag } from '$lib/state/tags';
import { isDefined } from '@gitbutler/ui/utils/typeguards';
import { createEntityAdapter, type EntityState } from '@reduxjs/toolkit';
import type { ForgeListingService } from '$lib/forge/interface/forgeListingService';
import type { PullRequest } from '$lib/forge/interface/types';
import type { GitHubApi } from '$lib/state/clientState.svelte';

export class GitHubListingService implements ForgeListingService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(gitHubApi: GitHubApi) {
		this.api = injectEndpoints(gitHubApi);
	}

	list(projectId: string, pollingInterval?: number) {
		return this.api.endpoints.listPrs.useQuery(projectId, {
			transform: (result) => prSelectors.selectAll(result),
			subscriptionOptions: { pollingInterval }
		});
	}

	getByBranch(projectId: string, branchName: string) {
		return this.api.endpoints.listPrs.useQuery(projectId, {
			transform: (result) => {
				return prSelectors.selectById(result, branchName);
			}
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

	async refresh(projectId: string): Promise<void> {
		await this.api.endpoints.listPrs.fetch(projectId, { forceRefetch: true });
	}
}

function injectEndpoints(api: GitHubApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			listPrs: build.query<EntityState<PullRequest, string>, string>({
				queryFn: async (_, api) => {
					const result = await ghQuery<'pulls', 'list', 'required'>(
						async (octokit, repository) => ({
							data: await octokit.paginate(octokit.rest.pulls.list, repository)
						}),
						api.extra,
						'required'
					);

					if (result.error) {
						return { error: result.error };
					}

					return {
						data: prAdapter.addMany(
							prAdapter.getInitialState(),
							result.data.map((item) => ghResponseToInstance(item))
						)
					};
				},
				providesTags: [providesList(ReduxTag.PullRequests)]
			}),
			listPrsByBranch: build.query<PullRequest | null, { projectId: string; branchName: string }>({
				queryFn: async ({ branchName }, api) => {
					const result = await ghQuery<'pulls', 'list', 'required'>(
						async (octokit, repository) => ({
							data: await octokit.paginate(octokit.rest.pulls.list, {
								...repository,
								head: `${repository.owner}:${branchName}`
							})
						}),
						api.extra,
						'required'
					);

					if (result.error) {
						return { error: result.error };
					}

					if (result.data.length === 0) {
						return { data: null };
					}

					if (result.data.length > 1) {
						return { error: new Error(`Multiple pull requests found for branch ${branchName}`) };
					}

					const prData = result.data[0]!;

					const pr = ghResponseToInstance(prData);
					return { data: pr };
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

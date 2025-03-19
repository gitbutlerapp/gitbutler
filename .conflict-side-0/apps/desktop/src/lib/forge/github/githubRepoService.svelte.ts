import { ghQuery } from './ghQuery';
import { ReduxTag } from '$lib/state/tags';
import type { ReactiveResult } from '$lib/state/butlerModule';
import type { GitHubApi } from '$lib/state/clientState.svelte';
import type { RepoResult } from './types';
import type { ForgeRepoService, RepoDetailedInfo } from '../interface/forgeRepoService';

export class GitHubRepoService implements ForgeRepoService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(gitHubApi: GitHubApi) {
		this.api = injectEndpoints(gitHubApi);
	}

	getInfo(): ReactiveResult<RepoDetailedInfo> {
		const result = $derived(
			this.api.endpoints.getRepos.useQuery(undefined, {
				transform: (result) => ({
					deleteBranchAfterMerge: result.delete_branch_on_merge
				})
			})
		);
		return result;
	}
}

function injectEndpoints(api: GitHubApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			getRepos: build.query<RepoResult, void>({
				queryFn: async (_, api) =>
					await ghQuery({
						domain: 'repos',
						action: 'get',
						extra: api.extra
					}),
				providesTags: [ReduxTag.PullRequests]
			})
		})
	});
}

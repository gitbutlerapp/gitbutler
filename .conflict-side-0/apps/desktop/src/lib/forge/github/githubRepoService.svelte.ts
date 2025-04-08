import { ghQuery } from '$lib/forge/github/ghQuery';
import { providesList, ReduxTag } from '$lib/state/tags';
import type { RepoResult } from '$lib/forge/github/types';
import type { ForgeRepoService, RepoDetailedInfo } from '$lib/forge/interface/forgeRepoService';
import type { ReactiveResult } from '$lib/state/butlerModule';
import type { GitHubApi } from '$lib/state/clientState.svelte';

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
				providesTags: [providesList(ReduxTag.PullRequests)]
			})
		})
	});
}

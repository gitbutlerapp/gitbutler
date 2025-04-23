import { ghQuery } from '$lib/forge/github/ghQuery';
import { providesList, ReduxTag } from '$lib/state/tags';
import type { RepoResult } from '$lib/forge/github/types';
import type { ForgeRepoService } from '$lib/forge/interface/forgeRepoService';
import type { GitHubApi } from '$lib/state/clientState.svelte';
import type { RepoInfo } from '$lib/url/gitUrl';

export class GitHubRepoService implements ForgeRepoService {
	readonly info: any;
	private api: ReturnType<typeof injectEndpoints>;

	constructor(
		gitHubApi: GitHubApi,
		private repo: RepoInfo
	) {
		this.api = injectEndpoints(gitHubApi);
	}

	getInfo() {
		const result = $derived(
			this.api.endpoints.getRepoInfo.useQuery(undefined, {
				transform: (info) => ({
					deleteBranchAfterMerge: !!info.delete_branch_on_merge
				})
			})
		);
		return result;
	}
}

function injectEndpoints(api: GitHubApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			getRepoInfo: build.query<RepoResult, void>({
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

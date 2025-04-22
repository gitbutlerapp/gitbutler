import { ghQuery } from '$lib/forge/github/ghQuery';
import { invalidatesList, ReduxTag } from '$lib/state/tags';
import type { CreateIssueResult } from '$lib/forge/github/types';
import type { ForgeIssueService } from '$lib/forge/interface/forgeIssueService';
import type { GitHubApi } from '$lib/state/clientState.svelte';

export class GitHubIssueService implements ForgeIssueService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(gitHubApi: GitHubApi) {
		this.api = injectEndpoints(gitHubApi);
	}

	async create(title: string, body: string, labels: string[]) {
		return await this.api.endpoints.create.mutate({ title, body, labels });
	}
}

function injectEndpoints(api: GitHubApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			create: build.mutation<CreateIssueResult, { title: string; body: string; labels: string[] }>({
				queryFn: async ({ title, body, labels }, api) =>
					await ghQuery({
						domain: 'issues',
						action: 'create',
						parameters: { title, body, labels },
						extra: api.extra
					}),
				invalidatesTags: [invalidatesList(ReduxTag.PullRequests)]
			})
		})
	});
}

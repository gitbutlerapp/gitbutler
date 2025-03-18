import { ghQuery } from './ghQuery';
import { ReduxTag } from '$lib/state/tags';
import type { ForgeIssueService } from '$lib/forge/interface/forgeIssueService';
import type { GitHubApi } from '$lib/state/clientState.svelte';
import type { CreateIssueResult } from './types';

export class GitHubIssueService implements ForgeIssueService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(gitHubApi: GitHubApi) {
		this.api = injectEndpoints(gitHubApi);
	}

	async create(title: string, body: string, labels: string[]) {
		return await this.api.endpoints.create.useMutation().triggerMutation({ title, body, labels });
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
				invalidatesTags: [ReduxTag.PullRequests]
			})
		})
	});
}

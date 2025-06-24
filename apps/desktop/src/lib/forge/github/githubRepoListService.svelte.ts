import { ghQuery } from '$lib/forge/github/ghQuery';
import { providesList, ReduxTag } from '$lib/state/tags';
import type { GitHubApi } from '$lib/state/clientState.svelte';
import type { RestEndpointMethodTypes } from '@octokit/rest';

export type GitHubRepository = RestEndpointMethodTypes['repos']['listForAuthenticatedUser']['response']['data'][number];

export class GitHubRepoListService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(gitHubApi: GitHubApi) {
		this.api = injectEndpoints(gitHubApi);
	}

	/**
	 * Fetch repositories for the authenticated user
	 */
	listUserRepos(params?: { per_page?: number; sort?: 'created' | 'updated' | 'pushed' | 'full_name'; type?: 'all' | 'owner' | 'public' | 'private' | 'member' }) {
		return this.api.endpoints.listUserRepos.useQuery(params || {});
	}

	/**
	 * Fetch repositories for the authenticated user (async version)
	 */
	async fetchUserRepos(params?: { per_page?: number; sort?: 'created' | 'updated' | 'pushed' | 'full_name'; type?: 'all' | 'owner' | 'public' | 'private' | 'member' }) {
		return await this.api.endpoints.listUserRepos.fetch(params || {});
	}
}

function injectEndpoints(api: GitHubApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			listUserRepos: build.query<GitHubRepository[], { per_page?: number; sort?: 'created' | 'updated' | 'pushed' | 'full_name'; type?: 'all' | 'owner' | 'public' | 'private' | 'member' }>({
				queryFn: async (params, api) => {
					const result = await ghQuery<'repos', 'listForAuthenticatedUser', 'optional'>(
						async (octokit) => ({
							data: await octokit.paginate(octokit.rest.repos.listForAuthenticatedUser, {
								per_page: 100,
								sort: 'updated',
								type: 'owner',
								...params
							})
						}),
						api.extra,
						'optional'
					);

					if (result.error) {
						return { error: result.error };
					}

					return { data: result.data };
				},
				providesTags: [providesList(ReduxTag.PullRequests)]
			})
		})
	});
}

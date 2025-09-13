import { ghQuery } from '$lib/forge/github/ghQuery';
import { GitHubBranch } from '$lib/forge/github/githubBranch';
import { GitHubChecksMonitor } from '$lib/forge/github/githubChecksMonitor.svelte';
import { GitHubListingService } from '$lib/forge/github/githubListingService.svelte';
import { GitHubPrService } from '$lib/forge/github/githubPrService.svelte';
import { GitHubRepoService } from '$lib/forge/github/githubRepoService.svelte';
import { GitHubIssueService } from '$lib/forge/github/issueService';
import { providesList, ReduxTag } from '$lib/state/tags';
import type { PostHogWrapper } from '$lib/analytics/posthog';
import type { GitHubClient } from '$lib/forge/github/githubClient';
import type { Forge, ForgeName } from '$lib/forge/interface/forge';
import type { ForgeArguments } from '$lib/forge/interface/types';
import type { GitHubApi } from '$lib/state/clientState.svelte';
import type { RestEndpointMethodTypes } from '@octokit/rest';
import type { TagDescription } from '@reduxjs/toolkit/query';

export const GITHUB_DOMAIN = 'github.com';

export class GitHub implements Forge {
	readonly name: ForgeName = 'github';
	readonly authenticated: boolean;
	private baseUrl: string;

	private api: ReturnType<typeof injectEndpoints>;

	constructor(
		private params: ForgeArguments & {
			posthog?: PostHogWrapper;
			client: GitHubClient;
			api: GitHubApi;
		}
	) {
		const { client, api, authenticated, repo } = params;
		const { owner, name } = repo;
		this.authenticated = authenticated;

		// Use the protocol from repo if available, otherwise default to https
		// For SSH remote URLs, always use HTTPS for browser compatibility
		let protocol = repo.protocol?.endsWith(':')
			? repo.protocol.slice(0, -1)
			: repo.protocol || 'https';

		// SSH URLs cannot be opened in browsers, so convert to HTTPS
		if (protocol === 'ssh') {
			protocol = 'https';
		}

		this.baseUrl = `${protocol}://${repo.domain}/${owner}/${name}`;

		this.api = injectEndpoints(api);

		// Reset the API when the token changes.
		client.onReset(() => api.util.resetApiState());
	}

	get listService() {
		if (!this.authenticated) return;
		const { api: gitHubApi } = this.params;
		return new GitHubListingService(gitHubApi);
	}

	get prService() {
		if (!this.authenticated) return;
		const { api: gitHubApi, posthog } = this.params;
		return new GitHubPrService(gitHubApi, posthog);
	}

	get repoService() {
		if (!this.authenticated) return;
		return new GitHubRepoService(this.params.api);
	}

	get issueService() {
		if (!this.authenticated) return;
		return new GitHubIssueService(this.params.api);
	}

	get checks() {
		if (!this.authenticated) return;
		return new GitHubChecksMonitor(this.params.api);
	}

	get user() {
		return this.api.endpoints.getGitHubUser.useQuery(null, {
			transform: (result) => ({
				id: result.id,
				name: result.name || result.login,
				srcUrl: result.avatar_url
			})
		});
	}

	branch(name: string) {
		const { baseBranch, forkStr } = this.params;
		if (!baseBranch) {
			return;
		}
		return new GitHubBranch(name, baseBranch, this.baseUrl, forkStr);
	}

	commitUrl(id: string): string {
		return `${this.baseUrl}/commit/${id}`;
	}

	invalidate(tags: TagDescription<ReduxTag>[]) {
		return this.params.api.util.invalidateTags(tags);
	}
}

type IsAuthenticated = RestEndpointMethodTypes['users']['getAuthenticated']['response']['data'];

function injectEndpoints(api: GitHubApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			getGitHubUser: build.query<IsAuthenticated, null>({
				queryFn: async (_, api) =>
					await ghQuery({
						domain: 'users',
						action: 'getAuthenticated',
						extra: api.extra
					}),
				providesTags: [providesList(ReduxTag.ForgeUser)]
			})
		})
	});
}

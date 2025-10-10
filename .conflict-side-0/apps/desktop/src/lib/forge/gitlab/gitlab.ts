import { GitLabBranch } from '$lib/forge/gitlab/gitlabBranch';
import { gitlab, type GitLabClient } from '$lib/forge/gitlab/gitlabClient.svelte';
import { GitLabListingService } from '$lib/forge/gitlab/gitlabListingService.svelte';
import { GitLabPrService } from '$lib/forge/gitlab/gitlabPrService.svelte';
import { providesList, ReduxTag } from '$lib/state/tags';
import { toSerializable } from '@gitbutler/shared/network/types';
import type { PostHogWrapper } from '$lib/analytics/posthog';
import type { Forge, ForgeName } from '$lib/forge/interface/forge';
import type { ForgeArguments, ForgeUser } from '$lib/forge/interface/types';
import type { GitLabApi } from '$lib/state/clientState.svelte';
import type { TagDescription } from '@reduxjs/toolkit/query';

export const GITLAB_DOMAIN = 'gitlab.com';
export const GITLAB_SUB_DOMAIN = 'gitlab'; // For self hosted instance of Gitlab

/**
 * PR support is pending OAuth support in the rust code.
 *
 * Follow this issue to stay in the loop:
 * https://github.com/gitbutlerapp/gitbutler/issues/2511
 */
export class GitLab implements Forge {
	readonly name: ForgeName = 'gitlab';
	readonly authenticated: boolean;
	private baseUrl: string;
	private baseBranch: string;
	private forkStr?: string;
	private api: ReturnType<typeof injectEndpoints>;

	constructor(
		private params: ForgeArguments & {
			posthog?: PostHogWrapper;
			api: GitLabApi;
			client: GitLabClient;
		}
	) {
		const { api, client, baseBranch, forkStr, authenticated, repo } = this.params;
		// Use the protocol from repo if available, otherwise default to https
		// For SSH remote URLs, always use HTTPS for browser compatibility
		let protocol = repo.protocol?.endsWith(':')
			? repo.protocol.slice(0, -1)
			: repo.protocol || 'https';

		// SSH URLs cannot be opened in browsers, so convert to HTTPS
		if (protocol === 'ssh') {
			protocol = 'https';
		}

		this.baseUrl = `${protocol}://${repo.domain}/${repo.owner}/${repo.name}`;
		this.baseBranch = baseBranch;
		this.forkStr = forkStr;
		this.authenticated = authenticated;

		this.api = injectEndpoints(api);

		// Reset the API when the token changes.
		client.onReset(() => api.util.resetApiState());
	}

	branch(name: string) {
		return new GitLabBranch(name, this.baseBranch, this.baseUrl, this.forkStr);
	}

	commitUrl(id: string): string {
		return `${this.baseUrl}/-/commit/${id}`;
	}

	get user() {
		return this.api.endpoints.getGitLabUser.useQuery();
	}

	get listService() {
		if (!this.authenticated) return;
		const { api: gitLabApi } = this.params;
		return new GitLabListingService(gitLabApi);
	}

	get issueService() {
		return undefined;
	}

	get prService() {
		if (!this.authenticated) return;
		const { api: gitLabApi, posthog } = this.params;
		return new GitLabPrService(gitLabApi, posthog);
	}

	get repoService() {
		return undefined;
	}

	get checks() {
		return undefined;
	}

	async pullRequestTemplateContent(_path?: string) {
		return undefined;
	}

	invalidate(tags: TagDescription<ReduxTag>[]) {
		return this.params.api.util.invalidateTags(tags);
	}
}
function injectEndpoints(api: GitLabApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			getGitLabUser: build.query<ForgeUser, void>({
				queryFn: async (args, query) => {
					try {
						const { api } = gitlab(query.extra);
						const user = await api.Users.showCurrentUser();
						const data = {
							id: user.id,
							name: user.name,
							srcUrl: user.avatar_url
						};
						return { data };
					} catch (e: unknown) {
						return { error: toSerializable(e) };
					}
				},
				providesTags: [providesList(ReduxTag.ForgeUser)]
			})
		})
	});
}

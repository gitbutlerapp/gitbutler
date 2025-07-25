import { GitLabBranch } from '$lib/forge/gitlab/gitlabBranch';
import { GitLabListingService } from '$lib/forge/gitlab/gitlabListingService.svelte';
import { GitLabPrService } from '$lib/forge/gitlab/gitlabPrService.svelte';
import type { PostHogWrapper } from '$lib/analytics/posthog';
import type { GitLabClient } from '$lib/forge/gitlab/gitlabClient.svelte';
import type { Forge, ForgeName } from '$lib/forge/interface/forge';
import type { ForgeArguments } from '$lib/forge/interface/types';
import type { GitLabApi } from '$lib/state/clientState.svelte';
import type { ReduxTag } from '$lib/state/tags';
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

	constructor(
		private params: ForgeArguments & {
			posthog?: PostHogWrapper;
			api: GitLabApi;
			client: GitLabClient;
		}
	) {
		const { api, client, baseBranch, forkStr, authenticated, repo } = this.params;
		this.baseUrl = `https://${repo.domain}/${repo.owner}/${repo.name}`;
		this.baseBranch = baseBranch;
		this.forkStr = forkStr;
		this.authenticated = authenticated;

		// Reset the API when the token changes.
		client.onReset(() => api.util.resetApiState());
	}

	branch(name: string) {
		return new GitLabBranch(name, this.baseBranch, this.baseUrl, this.forkStr);
	}

	commitUrl(id: string): string {
		return `${this.baseUrl}/-/commit/${id}`;
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

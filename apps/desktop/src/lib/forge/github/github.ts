import { GitHubBranch } from '$lib/forge/github/githubBranch';
import { GitHubChecksMonitor } from '$lib/forge/github/githubChecksMonitor.svelte';
import { GitHubListingService } from '$lib/forge/github/githubListingService.svelte';
import { GitHubPrService } from '$lib/forge/github/githubPrService.svelte';
import { GitHubRepoService } from '$lib/forge/github/githubRepoService.svelte';
import { GitHubIssueService } from '$lib/forge/github/issueService';
import type { PostHogWrapper } from '$lib/analytics/posthog';
import type { GitHubClient } from '$lib/forge/github/githubClient';
import type { Forge, ForgeName } from '$lib/forge/interface/forge';
import type { ForgeArguments } from '$lib/forge/interface/types';
import type { ProjectMetrics } from '$lib/metrics/projectMetrics';
import type { GitHubApi } from '$lib/state/clientState.svelte';
import type { ReduxTag } from '$lib/state/tags';
import type { TagDescription } from '@reduxjs/toolkit/query';

export const GITHUB_DOMAIN = 'github.com';

export class GitHub implements Forge {
	readonly name: ForgeName = 'github';
	readonly authenticated: boolean;
	private baseUrl: string;

	constructor(
		private params: ForgeArguments & {
			posthog?: PostHogWrapper;
			projectMetrics?: ProjectMetrics;
			client: GitHubClient;
			api: GitHubApi;
		}
	) {
		const { client, api, authenticated } = params;
		const { owner, name } = params.repo;
		this.authenticated = authenticated;
		this.baseUrl = `https://${GITHUB_DOMAIN}/${owner}/${name}`;

		// Reset the API when the token changes.
		client.onReset(() => api.util.resetApiState());
	}

	get listService() {
		const { api: gitHubApi, projectMetrics } = this.params;
		return new GitHubListingService(gitHubApi, projectMetrics);
	}

	get prService() {
		const { api: gitHubApi, posthog } = this.params;
		return new GitHubPrService(gitHubApi, posthog);
	}

	get repoService() {
		return new GitHubRepoService(this.params.api);
	}

	get issueService() {
		return new GitHubIssueService(this.params.api);
	}

	get checks() {
		return new GitHubChecksMonitor(this.params.api);
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

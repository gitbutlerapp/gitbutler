import { GitHubBranch } from '$lib/forge/github/githubBranch';
import { GitHubChecksMonitor } from '$lib/forge/github/githubChecksMonitor.svelte';
import { GitHubListingService } from '$lib/forge/github/githubListingService.svelte';
import { GitHubPrService } from '$lib/forge/github/githubPrService.svelte';
import { GitHubRepoService } from '$lib/forge/github/githubRepoService.svelte';
import { GitHubIssueService } from '$lib/forge/github/issueService';
import type { PostHogWrapper } from '$lib/analytics/posthog';
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
			gitHubApi: GitHubApi;
		}
	) {
		const { owner, name } = params.repo;
		this.authenticated = params.authenticated;
		this.baseUrl = `https://${GITHUB_DOMAIN}/${owner}/${name}`;
	}

	get listService() {
		const { gitHubApi, projectMetrics } = this.params;
		return new GitHubListingService(gitHubApi, projectMetrics);
	}

	get prService() {
		const { gitHubApi, posthog } = this.params;
		return new GitHubPrService(gitHubApi, posthog);
	}

	get repoService() {
		return new GitHubRepoService(this.params.gitHubApi);
	}

	get issueService() {
		return new GitHubIssueService(this.params.gitHubApi);
	}

	get checks() {
		return new GitHubChecksMonitor(this.params.gitHubApi);
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
		return this.params.gitHubApi.util.invalidateTags(tags);
	}
}

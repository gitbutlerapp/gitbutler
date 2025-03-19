import { GitHubBranch } from './githubBranch';
import { GitHubChecksMonitor } from './githubChecksMonitor.svelte';
import { GitHubListingService } from './githubListingService.svelte';
import { GitHubPrService } from './githubPrService.svelte';
import { GitHubRepoService } from './githubRepoService.svelte';
import { GitHubIssueService } from '$lib/forge/github/issueService';
import type { PostHogWrapper } from '$lib/analytics/posthog';
import type { ProjectMetrics } from '$lib/metrics/projectMetrics';
import type { GitHubApi } from '$lib/state/clientState.svelte';
import type { Forge, ForgeName } from '../interface/forge';
import type { ForgeArguments } from '../interface/types';

export const GITHUB_DOMAIN = 'github.com';

export class GitHub implements Forge {
	readonly name: ForgeName = 'github';
	private baseUrl: string;

	constructor(
		private params: ForgeArguments & {
			posthog?: PostHogWrapper;
			projectMetrics?: ProjectMetrics;
			gitHubApi: GitHubApi;
		}
	) {
		const { owner, name } = params.repo;
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

	checksMonitor(sourceBranch: string) {
		return new GitHubChecksMonitor(this.params.gitHubApi, sourceBranch);
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
}

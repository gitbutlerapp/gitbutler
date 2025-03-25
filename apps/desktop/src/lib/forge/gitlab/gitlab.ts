import { GitLabBranch } from '$lib/forge/gitlab/gitlabBranch';
import { GitLabPrService } from '$lib/forge/gitlab/gitlabPrService.svelte';
import type { PostHogWrapper } from '$lib/analytics/posthog';
import type { Forge, ForgeName } from '$lib/forge/interface/forge';
import type { DetailedPullRequest, ForgeArguments } from '$lib/forge/interface/types';
import type { ProjectMetrics } from '$lib/metrics/projectMetrics';
import type { GitLabApi } from '$lib/state/clientState.svelte';

export type PrAction = 'creating_pr';
export type PrState = { busy: boolean; branchId: string; action?: PrAction };
export type PrCacheKey = { value: DetailedPullRequest | undefined; fetchedAt: Date };

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
			projectMetrics?: ProjectMetrics;
			gitLabApi: GitLabApi;
		}
	) {
		const { baseBranch, forkStr, authenticated, repo } = this.params;
		this.baseUrl = `https://${repo.domain}/${repo.owner}/${repo.name}`;
		this.baseBranch = baseBranch;
		this.forkStr = forkStr;
		this.authenticated = authenticated;
	}

	branch(name: string) {
		return new GitLabBranch(name, this.baseBranch, this.baseUrl, this.forkStr);
	}

	commitUrl(id: string): string {
		return `${this.baseUrl}/-/commit/${id}`;
	}

	get listService() {
		return undefined;
	}

	get issueService() {
		return undefined;
	}

	get prService() {
		const { gitLabApi, posthog } = this.params;
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
}

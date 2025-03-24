import { AZURE_DOMAIN, AzureDevOps } from '$lib/forge/azure/azure';
import { BitBucket, BITBUCKET_DOMAIN } from '$lib/forge/bitbucket/bitbucket';
import { DefaultForge } from '$lib/forge/default.ts/default';
import { GitHub, GITHUB_DOMAIN } from '$lib/forge/github/github';
import { GitLab, GITLAB_DOMAIN, GITLAB_SUB_DOMAIN } from '$lib/forge/gitlab/gitlab';
import { ProjectMetrics } from '$lib/metrics/projectMetrics';
import type { PostHogWrapper } from '$lib/analytics/posthog';
import type { Forge } from '$lib/forge/interface/forge';
import type { GitHubApi } from '$lib/state/clientState.svelte';
import type { RepoInfo } from '$lib/url/gitUrl';
import type { Reactive } from '@gitbutler/shared/storeUtils';

// Used on a branch level to acquire the right kind of merge request / checks
// monitoring service.
export interface ForgeFactory {
	build(config: { repo: RepoInfo; pushRepo?: RepoInfo; baseBranch: string }): Forge | undefined;
}

export type ForgeConfig = {
	repo?: RepoInfo;
	pushRepo?: RepoInfo;
	baseBranch?: string;
	githubAuthenticated?: boolean;
};

export class DefaultForgeFactory implements ForgeFactory, Reactive<Forge> {
	private default = new DefaultForge();
	private _forge: Forge | undefined = $state();

	constructor(
		private gitHubApi: GitHubApi,
		private posthog: PostHogWrapper,
		private projectMetrics: ProjectMetrics
	) {}

	get current(): Forge {
		return this._forge || this.default;
	}

	setConfig(config: ForgeConfig) {
		const { repo, pushRepo, baseBranch, githubAuthenticated } = config;
		if (repo && baseBranch) {
			this._forge = this.build({ repo, pushRepo, baseBranch, githubAuthenticated });
		} else {
			this._forge = this.default;
		}
	}

	build({
		repo,
		pushRepo,
		baseBranch,
		githubAuthenticated
	}: {
		repo: RepoInfo;
		pushRepo?: RepoInfo;
		baseBranch: string;
		githubAuthenticated?: boolean;
	}): Forge {
		const domain = repo.domain;
		const forkStr =
			pushRepo && pushRepo.hash !== repo.hash ? `${pushRepo.owner}:${pushRepo.name}` : undefined;

		const baseParams = {
			repo,
			baseBranch,
			forkStr,
			authenticated: false
		};

		if (domain.includes(GITHUB_DOMAIN)) {
			return new GitHub({
				...baseParams,
				gitHubApi: this.gitHubApi,
				projectMetrics: this.projectMetrics,
				posthog: this.posthog,
				authenticated: !!githubAuthenticated
			});
		}
		if (domain === GITLAB_DOMAIN || domain.startsWith(GITLAB_SUB_DOMAIN + '.')) {
			return new GitLab(baseParams);
		}
		if (domain.includes(BITBUCKET_DOMAIN)) {
			return new BitBucket(baseParams);
		}
		if (domain.includes(AZURE_DOMAIN)) {
			return new AzureDevOps(baseParams);
		}
		return this.default;
	}
}

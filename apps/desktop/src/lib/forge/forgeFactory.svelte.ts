import { AZURE_DOMAIN, AzureDevOps } from '$lib/forge/azure/azure';
import { BitBucket, BITBUCKET_DOMAIN } from '$lib/forge/bitbucket/bitbucket';
import { DefaultForge } from '$lib/forge/default/default';
import { GitHub, GITHUB_DOMAIN } from '$lib/forge/github/github';
import { GitLab, GITLAB_DOMAIN, GITLAB_SUB_DOMAIN } from '$lib/forge/gitlab/gitlab';
import { ProjectMetrics } from '$lib/metrics/projectMetrics';
import type { PostHogWrapper } from '$lib/analytics/posthog';
import type { Forge } from '$lib/forge/interface/forge';
import type { GitHubApi, GitLabApi } from '$lib/state/clientState.svelte';
import type { ReduxTag } from '$lib/state/tags';
import type { RepoInfo } from '$lib/url/gitUrl';
import type { Reactive } from '@gitbutler/shared/storeUtils';
import type { ThunkDispatch, UnknownAction } from '@reduxjs/toolkit';
import type { TagDescription } from '@reduxjs/toolkit/query';

export type ForgeConfig = {
	repo?: RepoInfo;
	pushRepo?: RepoInfo;
	baseBranch?: string;
	githubAuthenticated?: boolean;
	gitlabAuthenticated?: boolean;
};

export class DefaultForgeFactory implements Reactive<Forge> {
	private default = new DefaultForge();
	private _forge: Forge | undefined = $state();

	constructor(
		private gitHubApi: GitHubApi,
		private gitLabApi: GitLabApi,
		private posthog: PostHogWrapper,
		private projectMetrics: ProjectMetrics,
		private dispatch: ThunkDispatch<any, any, UnknownAction>
	) {}

	get current(): Forge {
		return this._forge || this.default;
	}

	setConfig(config: ForgeConfig) {
		const { repo, pushRepo, baseBranch, githubAuthenticated, gitlabAuthenticated } = config;
		if (repo && baseBranch) {
			this._forge = this.build({
				repo,
				pushRepo,
				baseBranch,
				githubAuthenticated,
				gitlabAuthenticated
			});
		} else {
			this._forge = this.default;
		}
	}

	build({
		repo,
		pushRepo,
		baseBranch,
		githubAuthenticated,
		gitlabAuthenticated
	}: {
		repo: RepoInfo;
		pushRepo?: RepoInfo;
		baseBranch: string;
		githubAuthenticated?: boolean;
		gitlabAuthenticated?: boolean;
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
			return new GitLab({
				...baseParams,
				gitLabApi: this.gitLabApi,
				authenticated: !!gitlabAuthenticated
			});
		}
		if (domain.includes(BITBUCKET_DOMAIN)) {
			return new BitBucket(baseParams);
		}
		if (domain.includes(AZURE_DOMAIN)) {
			return new AzureDevOps(baseParams);
		}
		return this.default;
	}

	invalidate(tags: TagDescription<ReduxTag>[]) {
		const action = this.current.invalidate(tags);
		if (action) {
			this.dispatch(action);
		}
	}
}

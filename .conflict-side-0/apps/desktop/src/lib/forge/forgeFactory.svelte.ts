import { AZURE_DOMAIN, AzureDevOps } from '$lib/forge/azure/azure';
import { BitBucket, BITBUCKET_DOMAIN } from '$lib/forge/bitbucket/bitbucket';
import { DefaultForge } from '$lib/forge/default/default';
import { GitHub, GITHUB_DOMAIN } from '$lib/forge/github/github';
import { GitHubClient } from '$lib/forge/github/githubClient';
import { GitLab, GITLAB_DOMAIN, GITLAB_SUB_DOMAIN } from '$lib/forge/gitlab/gitlab';
import { ProjectMetrics } from '$lib/metrics/projectMetrics';
import type { PostHogWrapper } from '$lib/analytics/posthog';
import type { GitLabClient } from '$lib/forge/gitlab/gitlabClient.svelte';
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
		private params: {
			gitHubClient: GitHubClient;
			gitHubApi: GitHubApi;
			gitLabClient: GitLabClient;
			gitLabApi: GitLabApi;
			posthog: PostHogWrapper;
			projectMetrics: ProjectMetrics;
			dispatch: ThunkDispatch<any, any, UnknownAction>;
		}
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
			const { gitHubClient, gitHubApi, posthog, projectMetrics } = this.params;
			return new GitHub({
				...baseParams,
				api: gitHubApi,
				client: gitHubClient,
				posthog: posthog,
				projectMetrics: projectMetrics,
				authenticated: !!githubAuthenticated
			});
		}
		if (domain === GITLAB_DOMAIN || domain.startsWith(GITLAB_SUB_DOMAIN + '.')) {
			const { gitLabClient, gitLabApi, posthog } = this.params;
			return new GitLab({
				...baseParams,
				api: gitLabApi,
				client: gitLabClient,
				posthog: posthog,
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
		const { dispatch } = this.params;
		if (action) {
			dispatch(action);
		}
	}
}

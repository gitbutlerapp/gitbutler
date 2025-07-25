import { AZURE_DOMAIN, AzureDevOps } from '$lib/forge/azure/azure';
import { BitBucket, BITBUCKET_DOMAIN } from '$lib/forge/bitbucket/bitbucket';
import { DefaultForge } from '$lib/forge/default/default';
import { GitHub, GITHUB_DOMAIN } from '$lib/forge/github/github';
import { GitHubClient } from '$lib/forge/github/githubClient';
import { GitLab, GITLAB_DOMAIN, GITLAB_SUB_DOMAIN } from '$lib/forge/gitlab/gitlab';
import { InjectionToken } from '@gitbutler/shared/context';
import { BehaviorSubject } from 'rxjs';
import type { PostHogWrapper } from '$lib/analytics/posthog';
import type { GitLabClient } from '$lib/forge/gitlab/gitlabClient.svelte';
import type { Forge, ForgeName } from '$lib/forge/interface/forge';
import type { ReadonlyBehaviorSubject } from '$lib/rxjs';
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
	forgeOverride?: ForgeName;
};

export const DEFAULT_FORGE_FACTORY = new InjectionToken<DefaultForgeFactory>('DefaultForgeFactory');

export class DefaultForgeFactory implements Reactive<Forge> {
	private default = new DefaultForge();
	private _forge = $state<Forge>(this.default);
	private readonly _determinedForgeType = new BehaviorSubject<ForgeName>('default');

	constructor(
		private params: {
			gitHubClient: GitHubClient;
			gitHubApi: GitHubApi;
			gitLabClient: GitLabClient;
			gitLabApi: GitLabApi;
			posthog: PostHogWrapper;
			dispatch: ThunkDispatch<any, any, UnknownAction>;
		}
	) {}

	get current(): Forge {
		return this._forge;
	}

	get determinedForgeType(): ReadonlyBehaviorSubject<ForgeName> {
		return this._determinedForgeType;
	}

	setConfig(config: ForgeConfig) {
		const { repo, pushRepo, baseBranch, githubAuthenticated, gitlabAuthenticated, forgeOverride } =
			config;
		if (repo && baseBranch) {
			this._determinedForgeType.next(this.determineForgeType(repo));
			this._forge = this.build({
				repo,
				pushRepo,
				baseBranch,
				githubAuthenticated,
				gitlabAuthenticated,
				forgeOverride
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
		gitlabAuthenticated,
		forgeOverride
	}: {
		repo: RepoInfo;
		pushRepo?: RepoInfo;
		baseBranch: string;
		githubAuthenticated?: boolean;
		gitlabAuthenticated?: boolean;
		forgeOverride: ForgeName | undefined;
	}): Forge {
		let forgeType = this.determineForgeType(repo);
		if (forgeType === 'default' && forgeOverride) {
			forgeType = forgeOverride;
		}
		const forkStr =
			pushRepo && pushRepo.hash !== repo.hash ? `${pushRepo.owner}:${pushRepo.name}` : undefined;

		const baseParams = {
			repo,
			baseBranch,
			forkStr,
			authenticated: false
		};

		if (forgeType === 'github') {
			const { gitHubClient, gitHubApi, posthog } = this.params;
			return new GitHub({
				...baseParams,
				api: gitHubApi,
				client: gitHubClient,
				posthog: posthog,
				authenticated: !!githubAuthenticated
			});
		}
		if (forgeType === 'gitlab') {
			const { gitLabClient, gitLabApi, posthog } = this.params;
			return new GitLab({
				...baseParams,
				api: gitLabApi,
				client: gitLabClient,
				posthog: posthog,
				authenticated: !!gitlabAuthenticated
			});
		}
		if (forgeType === 'bitbucket') {
			return new BitBucket(baseParams);
		}
		if (forgeType === 'azure') {
			return new AzureDevOps(baseParams);
		}
		return this.default;
	}

	private determineForgeType(repo: RepoInfo): ForgeName {
		const domain = repo.domain;

		if (domain.includes(GITHUB_DOMAIN)) {
			return 'github';
		}
		if (
			domain === GITLAB_DOMAIN ||
			domain.startsWith(GITLAB_SUB_DOMAIN + '.') ||
			domain.startsWith('xy' + GITLAB_SUB_DOMAIN + '.') // Temporary workaround until we have foerge overrides implemented
		) {
			return 'gitlab';
		}
		if (domain.includes(BITBUCKET_DOMAIN)) {
			return 'bitbucket';
		}
		if (domain.includes(AZURE_DOMAIN)) {
			return 'azure';
		}

		return 'default';
	}

	invalidate(tags: TagDescription<ReduxTag>[]) {
		const action = this.current.invalidate(tags);
		const { dispatch } = this.params;
		if (action) {
			dispatch(action);
		}
	}
}

import { AZURE_DOMAIN, AzureDevOps } from '$lib/forge/azure/azure';
import { BitBucket, BITBUCKET_DOMAIN } from '$lib/forge/bitbucket/bitbucket';
import { DefaultForge } from '$lib/forge/default/default';
import { GitHub, GITHUB_DOMAIN } from '$lib/forge/github/github';
import { GitHubClient } from '$lib/forge/github/githubClient';
import { GitLab, GITLAB_DOMAIN, GITLAB_SUB_DOMAIN } from '$lib/forge/gitlab/gitlab';
import { InjectionToken } from '@gitbutler/core/context';
import { deepCompare } from '@gitbutler/shared/compare';
import type { PostHogWrapper } from '$lib/analytics/posthog';
import type { ForgeProvider } from '$lib/baseBranch/baseBranch';
import type { GitLabClient } from '$lib/forge/gitlab/gitlabClient.svelte';
import type { Forge, ForgeName } from '$lib/forge/interface/forge';
import type { BackendApi, GitHubApi, GitLabApi } from '$lib/state/clientState.svelte';
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
	githubIsLoading?: boolean;
	githubError?: { code: string; message: string };
	gitlabAuthenticated?: boolean;
	detectedForgeProvider: ForgeProvider | undefined;
	forgeOverride?: ForgeName;
};

export const DEFAULT_FORGE_FACTORY = new InjectionToken<DefaultForgeFactory>('DefaultForgeFactory');

export class DefaultForgeFactory implements Reactive<Forge> {
	private default = new DefaultForge();
	private _forge = $state<Forge>(this.default);
	private _config: any = undefined;
	private _determinedForgeType = $state<ForgeName>('default');
	private _githubError = $state<{ code: string; message: string } | undefined>(undefined);
	private _canSetupIntegration = $derived.by(() => {
		// Don't show the setup prompt if there's a network error
		if (this._githubError?.code === 'errors.network') {
			return undefined;
		}
		return isAvalilableForge(this._determinedForgeType) &&
			!this._forge.authenticated &&
			!this._forge.isLoading
			? this._determinedForgeType
			: undefined;
	});

	constructor(
		private params: {
			backendApi: BackendApi;
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

	get determinedForgeType(): ForgeName {
		return this._determinedForgeType;
	}

	get canSetupIntegration(): AvailableForge | undefined {
		return this._canSetupIntegration;
	}

	setConfig(config: ForgeConfig) {
		if (deepCompare(config, this._config)) {
			return;
		}
		this._config = config;
		const {
			repo,
			pushRepo,
			baseBranch,
			githubAuthenticated,
			githubIsLoading,
			githubError,
			gitlabAuthenticated,
			detectedForgeProvider,
			forgeOverride
		} = config;
		this._githubError = githubError;
		if (repo && baseBranch) {
			this._determinedForgeType = this.determineForgeType(repo, detectedForgeProvider);
			this._forge = this.build({
				repo,
				pushRepo,
				baseBranch,
				githubAuthenticated,
				githubIsLoading,
				gitlabAuthenticated,
				detectedForgeProvider,
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
		githubIsLoading,
		gitlabAuthenticated,
		detectedForgeProvider,
		forgeOverride
	}: {
		repo: RepoInfo;
		pushRepo?: RepoInfo;
		baseBranch: string;
		githubAuthenticated?: boolean;
		githubIsLoading?: boolean;
		gitlabAuthenticated?: boolean;
		detectedForgeProvider: ForgeProvider | undefined;
		forgeOverride: ForgeName | undefined;
	}): Forge {
		let forgeType = this.determineForgeType(repo, detectedForgeProvider);
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
			const { gitHubClient, gitHubApi, posthog, backendApi } = this.params;
			return new GitHub({
				...baseParams,
				api: gitHubApi,
				backendApi,
				client: gitHubClient,
				posthog: posthog,
				authenticated: !!githubAuthenticated,
				isLoading: githubIsLoading ?? false
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

	private determineForgeType(
		repo: RepoInfo,
		detectedForgeProvider: ForgeProvider | undefined
	): ForgeName {
		if (detectedForgeProvider) {
			return detectedForgeProvider;
		}
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

const AVAILABLE_FORGES = ['github', 'gitlab'] satisfies ForgeName[];
export type AvailableForge = (typeof AVAILABLE_FORGES)[number];

function isAvalilableForge(forge: ForgeName): forge is AvailableForge {
	return AVAILABLE_FORGES.includes(forge as AvailableForge);
}

export function availableForgeLabel(forge: AvailableForge): string {
	switch (forge) {
		case 'github':
			return 'GitHub';
		case 'gitlab':
			return 'GitLab';
	}
}

export function availableForgeReviewUnit(forge: AvailableForge): string {
	switch (forge) {
		case 'github':
			return 'Pull Requests';
		case 'gitlab':
			return 'Merge Requests';
	}
}

export function availableForgeDocsLink(forge: AvailableForge): string {
	switch (forge) {
		case 'github':
			return 'https://docs.gitbutler.com/features/forge-integration/github-integration';
		case 'gitlab':
			return 'https://docs.gitbutler.com/features/forge-integration/gitlab-integration';
	}
}

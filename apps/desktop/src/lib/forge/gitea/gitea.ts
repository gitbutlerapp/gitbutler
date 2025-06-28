import type { PostHogWrapper } from '$lib/analytics/posthog';
import type { Forge, ForgeName } from '$lib/forge/interface/forge';
import type { DetailedPullRequest, ForgeArguments } from '$lib/forge/interface/types';
import type { ProjectMetrics } from '$lib/metrics/projectMetrics';
import type { GiteaApi, GitLabApi } from '$lib/state/clientState.svelte';
import type { ReduxTag } from '$lib/state/tags';
import type { TagDescription } from '@reduxjs/toolkit/query';
import { gitea, type GiteaClient } from '$lib/forge/gitea/giteaClient.svelte';
import { GiteaListingService } from '$lib/forge/gitea/gitlabListingService.svelte';
import { GiteaPrService } from '$lib/forge/gitea/giteaPrService.svelte';
import { GiteaBranch } from '$lib/forge/gitea/giteaBranch';
import { isValidGiteaProjectId } from '$lib/forge/gitea/types';

export type PrAction = 'creating_pr';
export type PrState = { busy: boolean; branchId: string; action?: PrAction };
export type PrCacheKey = { value: DetailedPullRequest | undefined; fetchedAt: Date };

export const GITEA_DOMAIN = 'gitea.com';
export const GITEA_SUB_DOMAIN = 'gitea'; // For self hosted instance of Gitlab

/**
 * PR support is pending OAuth support in the rust code.
 *
 * Follow this issue to stay in the loop:
 * https://github.com/gitbutlerapp/gitbutler/issues/2511
 */
export class Gitea implements Forge {
	readonly name: ForgeName = 'gitea';
	readonly authenticated: boolean;
	private baseUrl: string;
	private baseBranch: string;
	private forkStr?: string;

	constructor(
		private params: ForgeArguments & {
			posthog?: PostHogWrapper;
			projectMetrics?: ProjectMetrics;
			api: GiteaApi;
			client: GiteaClient;
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
		return new GiteaBranch(name, this.baseBranch, this.baseUrl, this.forkStr);
	}

	commitUrl(id: string): string {
		return `${this.baseUrl}/-/commit/${id}`;
	}

	get listService() {
		const { api: giteaApi, projectMetrics } = this.params;
		return new GiteaListingService(giteaApi, projectMetrics);
	}

	get issueService() {
		return undefined;
	}

	get prService() {
		const { api: giteaApi, posthog } = this.params;
		return new GiteaPrService(giteaApi, posthog);
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

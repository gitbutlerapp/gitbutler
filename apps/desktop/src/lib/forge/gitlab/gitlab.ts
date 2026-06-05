import { GitLabBranch } from "$lib/forge/gitlab/gitlabBranch";
import { type GitLabClient } from "$lib/forge/gitlab/gitlabClient.svelte";
import { ListingService } from "$lib/forge/shared/listingService.svelte";
import { PrService } from "$lib/forge/shared/prService.svelte";
import { RepoService } from "$lib/forge/shared/repoService.svelte";
import { ReduxTag } from "$lib/state/tags";
import type { Forge, ForgeName } from "$lib/forge/interface/forge";
import type { ForgeArguments } from "$lib/forge/interface/types";
import type { BackendApi } from "$lib/state/backendApi";
import type { AppDispatch, GitLabApi } from "$lib/state/clientState.svelte";
import type { PostHogWrapper } from "$lib/telemetry/posthog";
import type { Branded } from "@gitbutler/shared/utils/branding";
import type { TagDescription } from "@reduxjs/toolkit/query";

export const GITLAB_DOMAIN = "gitlab.com";
export const GITLAB_SUB_DOMAIN = "gitlab"; // For self hosted instance of Gitlab

/**
 * PR support is pending OAuth support in the rust code.
 *
 * Follow this issue to stay in the loop:
 * https://github.com/gitbutlerapp/gitbutler/issues/2511
 */
export class GitLab implements Forge {
	readonly name: ForgeName = "gitlab";
	readonly authenticated: boolean;
	readonly isLoading: boolean;
	private baseUrl: string;
	private baseBranch: string;
	private forkStr?: string;

	constructor(
		private params: ForgeArguments & {
			posthog?: PostHogWrapper;
			api: GitLabApi;
			backendApi: BackendApi;
			client: GitLabClient;
			dispatch: AppDispatch;
			isLoading: boolean;
		},
	) {
		const { baseBranch, forkStr, authenticated, repo, isLoading } = this.params;
		// Use the protocol from repo if available, otherwise default to https
		// For SSH remote URLs, always use HTTPS for browser compatibility
		let protocol = repo.protocol?.endsWith(":")
			? repo.protocol.slice(0, -1)
			: repo.protocol || "https";

		// SSH URLs cannot be opened in browsers, so convert to HTTPS
		if (protocol === "ssh") {
			protocol = "https";
		}

		this.baseUrl = `${protocol}://${repo.domain}/${repo.owner}/${repo.name}`;
		this.baseBranch = baseBranch;
		this.forkStr = forkStr;
		this.authenticated = authenticated;
		this.isLoading = isLoading;
	}

	branch(name: string) {
		return new GitLabBranch(name, this.baseBranch, this.baseUrl, this.forkStr);
	}

	commitUrl(id: string): string {
		return `${this.baseUrl}/-/commit/${id}`;
	}

	prUrl(number: number): string {
		return `${this.baseUrl}/-/merge_requests/${number}`;
	}

	get listService() {
		if (!this.authenticated) return;
		const { backendApi, dispatch } = this.params;
		return new ListingService(backendApi, dispatch);
	}

	get prService() {
		if (!this.authenticated) return;
		const { posthog, backendApi } = this.params;
		return new PrService(
			{ name: "Merge request", abbr: "MR", symbol: "!" },
			"Gitlab MR",
			backendApi,
			posthog,
		);
	}

	get repoService() {
		if (!this.authenticated) return;
		const { backendApi, repo } = this.params;
		return new RepoService(backendApi, repo.owner, repo.name);
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

export type GitLabProjectId = Branded<string, "GitLabProjectId">;

export function createGitLabProjectId(owner: string, name: string): GitLabProjectId {
	return encodeURI(`${owner}/${name}`) as GitLabProjectId;
}

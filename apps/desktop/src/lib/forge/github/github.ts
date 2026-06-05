import { GitHubBranch } from "$lib/forge/github/githubBranch";
import { GitHubChecksMonitor } from "$lib/forge/github/githubChecksMonitor.svelte";
import { ListingService } from "$lib/forge/shared/listingService.svelte";
import { PrService } from "$lib/forge/shared/prService.svelte";
import { RepoService } from "$lib/forge/shared/repoService.svelte";
import { ReduxTag } from "$lib/state/tags";
import type { GitHubClient } from "$lib/forge/github/githubClient";
import type { Forge, ForgeName } from "$lib/forge/interface/forge";
import type { ForgeArguments } from "$lib/forge/interface/types";
import type { BackendApi } from "$lib/state/backendApi";
import type { AppDispatch, GitHubApi } from "$lib/state/clientState.svelte";
import type { PostHogWrapper } from "$lib/telemetry/posthog";
import type { TagDescription } from "@reduxjs/toolkit/query";

export const GITHUB_DOMAIN = "github.com";

export class GitHub implements Forge {
	readonly name: ForgeName = "github";
	readonly authenticated: boolean;
	readonly isLoading: boolean;
	private baseUrl: string;

	constructor(
		private params: ForgeArguments & {
			dispatch: AppDispatch;
			posthog?: PostHogWrapper;
			client: GitHubClient;
			api: GitHubApi;
			backendApi: BackendApi;
			isLoading: boolean;
		},
	) {
		const { client, api, authenticated, repo, isLoading } = params;
		const { owner, name } = repo;
		this.authenticated = authenticated;
		this.isLoading = isLoading;

		// Use the protocol from repo if available, otherwise default to https
		// For SSH remote URLs, always use HTTPS for browser compatibility
		let protocol = repo.protocol?.endsWith(":")
			? repo.protocol.slice(0, -1)
			: repo.protocol || "https";

		// SSH URLs cannot be opened in browsers, so convert to HTTPS
		if (protocol === "ssh") {
			protocol = "https";
		}

		this.baseUrl = `${protocol}://${repo.domain}/${owner}/${name}`;

		// Reset the API when the token changes.
		client.onReset(() => api.util.resetApiState());
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
			{ name: "Pull request", abbr: "PR", symbol: "#" },
			"PR",
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
		if (!this.authenticated) return;
		return new GitHubChecksMonitor(this.params.backendApi);
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

	prUrl(number: number): string {
		return `${this.baseUrl}/pull/${number}`;
	}

	invalidate(tags: TagDescription<ReduxTag>[]) {
		return this.params.api.util.invalidateTags(tags);
	}
}

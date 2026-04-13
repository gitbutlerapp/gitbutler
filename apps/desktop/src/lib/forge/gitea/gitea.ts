import { GiteaPrService } from "$lib/forge/gitea/giteaPrService.svelte";
import { GiteaRepoService } from "$lib/forge/gitea/giteaRepoService.svelte";
import { providesList, ReduxTag } from "$lib/state/tags";
import type { GiteaClient } from "$lib/forge/gitea/giteaClient.svelte";
import type { Forge, ForgeName } from "$lib/forge/interface/forge";
import type { ForgeArguments } from "$lib/forge/interface/types";
import type { AppDispatch, BackendApi, GiteaApi } from "$lib/state/clientState.svelte";
import type { PostHogWrapper } from "$lib/telemetry/posthog";
import type { TagDescription } from "@reduxjs/toolkit/query";

export class Gitea implements Forge {
	readonly name: ForgeName = "gitea";
	readonly authenticated: boolean;
	readonly isLoading: boolean;
	private baseUrl: string;

	private api: ReturnType<typeof injectEndpoints>;

	constructor(
		private params: ForgeArguments & {
			dispatch: AppDispatch;
			posthog?: PostHogWrapper;
			client: GiteaClient;
			api: GiteaApi;
			backendApi: BackendApi;
			isLoading: boolean;
		},
	) {
		const { client, api, authenticated, repo, isLoading } = params;
		const { owner, name } = repo;
		this.authenticated = authenticated;
		this.isLoading = isLoading;

		let protocol = repo.protocol?.endsWith(":")
			? repo.protocol.slice(0, -1)
			: repo.protocol || "https";

		if (protocol === "ssh") {
			protocol = "https";
		}

		this.baseUrl = `${protocol}://${repo.domain}/${owner}/${name}`;

		this.api = injectEndpoints(api);

		// Initialize the client with repo info
		client.set(
			`${protocol}://${repo.domain}`,
			undefined, // Token is handled via backend commands mostly, but can be set if needed
			owner,
			name
		);
	}

	get prService() {
		if (!this.authenticated) return;
		const { api: giteaApi, posthog, backendApi } = this.params;
		return new GiteaPrService(giteaApi, backendApi, posthog);
	}

	get repoService() {
		if (!this.authenticated) return;
		return new GiteaRepoService(this.params.api);
	}

	get user() {
		return this.api.endpoints.getGiteaUser.useQuery(null, {
			transform: (result) => ({
				id: result.id,
				name: result.name || result.login,
				srcUrl: result.avatar_url,
			}),
		});
	}

	branch(name: string) {
		// Basic branch URL for Gitea
		return {
			url: `${this.baseUrl}/src/branch/${name}`,
		};
	}

	commitUrl(id: string): string {
		return `${this.baseUrl}/commit/${id}`;
	}

	invalidate(tags: TagDescription<ReduxTag>[]) {
		return this.params.api.util.invalidateTags(tags);
	}
}

function injectEndpoints(api: GiteaApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			getGiteaUser: build.query<any, null>({
				queryFn: async (_, api) => {
					// This should call the backend to get the user info for the currently active account
					// For now, we'll assume the backend command handles it or we'll need to pass the account ID
					return { error: { name: "Gitea API error", message: "Not implemented" } };
				},
				providesTags: [providesList(ReduxTag.ForgeUser)],
			}),
		}),
	});
}

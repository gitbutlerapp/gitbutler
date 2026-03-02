import { GiteaBranch } from "$lib/forge/gitea/giteaBranch";
import { gitea, type GiteaClient } from "$lib/forge/gitea/giteaClient.svelte";
import { GiteaListingService } from "$lib/forge/gitea/giteaListingService.svelte";
import { GiteaPrService } from "$lib/forge/gitea/giteaPrService.svelte";
import { isValidGiteaProjectId } from "$lib/forge/gitea/types";
import { providesList, ReduxTag } from "$lib/state/tags";
import { toSerializable } from "@gitbutler/shared/network/types";
import type { PostHogWrapper } from "$lib/analytics/posthog";
import type { Forge, ForgeName } from "$lib/forge/interface/forge";
import type { ForgeArguments, ForgeUser } from "$lib/forge/interface/types";
import type { GiteaApi } from "$lib/state/clientState.svelte";
import type { ReduxTag as ReduxTagType } from "$lib/state/tags";
import type { PayloadAction } from "@reduxjs/toolkit";
import type { TagDescription } from "@reduxjs/toolkit/query";
import type { ThunkDispatch, UnknownAction } from "@reduxjs/toolkit";

export const GITEA_DOMAIN = "gitea.com";
export const GITEA_SUB_DOMAIN = "gitea";

export class Gitea implements Forge {
	readonly name: ForgeName = "gitea";
	readonly authenticated: boolean;
	readonly isLoading: boolean = false;
	private baseUrl: string;
	private baseBranch: string;
	private forkStr?: string;
	private api: ReturnType<typeof injectEndpoints>;

	constructor(
		private params: ForgeArguments & {
			posthog?: PostHogWrapper;
			api: GiteaApi;
			client: GiteaClient;
			dispatch: ThunkDispatch<any, any, UnknownAction>;
		},
	) {
		const { api, baseBranch, forkStr, authenticated, repo } = this.params;

		let protocol = repo.protocol?.endsWith(":")
			? repo.protocol.slice(0, -1)
			: repo.protocol || "https";

		if (protocol === "ssh") {
			protocol = "https";
		}

		this.baseUrl = `${protocol}://${repo.domain}/${repo.owner}/${repo.name}`;
		this.baseBranch = baseBranch;
		this.forkStr = forkStr;
		this.authenticated = authenticated;

		this.api = injectEndpoints(api);
	}

	branch(name: string) {
		return new GiteaBranch(name, this.baseBranch, this.baseUrl, this.forkStr);
	}

	commitUrl(id: string): string {
		return `${this.baseUrl}/commit/${id}`;
	}

	get user() {
		return this.api.endpoints.getGiteaUser.useQuery();
	}

	get listService() {
		if (!this.authenticated) return undefined;
		const { api, dispatch } = this.params;
		return new GiteaListingService(api, dispatch);
	}

	get issueService() {
		return undefined;
	}

	get prService() {
		if (!this.authenticated) return undefined;
		const { api, posthog } = this.params;
		return new GiteaPrService(api, posthog);
	}

	get repoService() {
		return undefined;
	}

	get checks() {
		return undefined;
	}

	invalidate(tags: TagDescription<ReduxTagType>[]): PayloadAction<any> | undefined {
		return this.params.api.util.invalidateTags(tags);
	}
}

function injectEndpoints(api: GiteaApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			getGiteaUser: build.query<ForgeUser, void>({
				queryFn: async (_args, query) => {
					try {
						const { api } = gitea(query.extra);
						const response = await api.user.userGetCurrent();
						const user = response.data;
						const data: ForgeUser = {
							id: user.id || 0,
							name: user.full_name || user.login || "",
							srcUrl: user.avatar_url || "",
						};
						return { data };
					} catch (e: unknown) {
						return { error: toSerializable(e) };
					}
				},
				providesTags: [providesList(ReduxTag.ForgeUser)],
			}),
		}),
	});
}

export function isGiteaProjectIdValid(owner?: string, repo?: string): boolean {
	if (!owner || !repo) return false;
	return isValidGiteaProjectId(`${owner}/${repo}`);
}

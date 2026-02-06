import { GiteaBranch } from '$lib/forge/gitea/giteaBranch';
import { gitea, type GiteaClient } from '$lib/forge/gitea/giteaClient.svelte';
import { GiteaListingService } from '$lib/forge/gitea/giteaListingService.svelte';
import { GiteaPrService } from '$lib/forge/gitea/giteaPrService.svelte';
import { providesList, ReduxTag } from '$lib/state/tags';
import { toSerializable } from '@gitbutler/shared/network/types';
import type { PostHogWrapper } from '$lib/analytics/posthog';
import type { Forge, ForgeName } from '$lib/forge/interface/forge';
import type { ForgeArguments, ForgeUser } from '$lib/forge/interface/types';
import type { GiteaApi } from '$lib/state/clientState.svelte';
import type { ThunkDispatch, UnknownAction } from '@reduxjs/toolkit';
import type { TagDescription } from '@reduxjs/toolkit/query';

export const GITEA_DOMAIN = 'codeberg.org';
export const GITEA_SUB_DOMAIN = 'gitea';

export class Gitea implements Forge {
	readonly name: ForgeName = 'gitea';
	readonly authenticated: boolean;
	readonly isLoading: boolean;
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
		}
	) {
		const { api, client, baseBranch, forkStr, authenticated, repo } = this.params;

		let protocol = repo.protocol?.endsWith(':')
			? repo.protocol.slice(0, -1)
			: repo.protocol || 'https';

		if (protocol === 'ssh') {
			protocol = 'https';
		}

		this.baseUrl = `${protocol}://${repo.domain}/${repo.owner}/${repo.name}`;
		this.baseBranch = baseBranch;
		this.forkStr = forkStr;
		this.authenticated = authenticated;
		this.isLoading = false;

		this.api = injectEndpoints(api);

		client.onReset(() => api.util.resetApiState());
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
		if (!this.authenticated) return;
		const { api: giteaApi, dispatch } = this.params;
		return new GiteaListingService(giteaApi, dispatch);
	}

	get issueService() {
		return undefined;
	}

	get prService() {
		if (!this.authenticated) return;
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

function injectEndpoints(api: GiteaApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			getGiteaUser: build.query<ForgeUser, void>({
				queryFn: async (args, query) => {
					try {
						const { client } = gitea(query.extra);
						const user = await client.getCurrentUser();
						const data = {
							id: user.id,
							name: user.full_name || user.login,
							srcUrl: user.avatar_url
						};
						return { data };
					} catch (e: unknown) {
						return { error: toSerializable(e) };
					}
				},
				providesTags: [providesList(ReduxTag.ForgeUser)]
			})
		})
	});
}

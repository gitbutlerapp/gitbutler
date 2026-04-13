import { gitea } from "$lib/forge/gitea/giteaClient.svelte";
import { providesList, ReduxTag } from "$lib/state/tags";
import type { ForgeRepoService, RepoDetailedInfo } from "$lib/forge/interface/forgeRepoService";
import type { ReactiveQuery } from "$lib/state/butlerModule";
import type { GiteaApi } from "$lib/state/clientState.svelte";

export class GiteaRepoService implements ForgeRepoService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(giteaApi: GiteaApi) {
		this.api = injectEndpoints(giteaApi);
	}

	getInfo(): ReactiveQuery<RepoDetailedInfo> {
		return this.api.endpoints.getRepo.useQuery(undefined, {
			transform: (result) => ({
				deleteBranchAfterMerge: result.ignore_whitespace_conflicts || false, // Gitea might not have this exact field
			}),
		});
	}
}

function injectEndpoints(api: GiteaApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			getRepo: build.query<any, void>({
				queryFn: async (_, api) => {
					const client = gitea(api.extra);
					const response = await client.fetch(`/repos/${client.owner}/${client.repo}`);
					if (!response.ok) {
						return { error: { name: "Gitea API error", message: `Failed to fetch repo info: ${response.status}` } };
					}
					const data = await response.json();
					return { data };
				},
				providesTags: [providesList(ReduxTag.GiteaPRs)],
			}),
		}),
	});
}

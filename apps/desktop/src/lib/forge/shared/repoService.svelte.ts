import { providesList, ReduxTag } from "$lib/state/tags";
import type { ForgeRepoService, RepoDetailedInfo } from "$lib/forge/interface/forgeRepoService";
import type { BackendApi } from "$lib/state/backendApi";
import type { ReactiveQuery } from "$lib/state/butlerModule";
import type { RepoInfo } from "@gitbutler/but-sdk";

export class RepoService implements ForgeRepoService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(
		backendApi: BackendApi,
		private readonly owner: string,
		private readonly repo: string,
	) {
		this.api = injectEndpoints(backendApi);
	}

	getInfo(projectId: string): ReactiveQuery<RepoDetailedInfo> {
		return this.api.endpoints.getRepoInfo.useQuery(
			{ projectId, owner: this.owner, repo: this.repo },
			{
				transform: (result) => ({
					deleteBranchAfterMerge: result.deleteBranchOnMerge ?? undefined,
					fork: result.fork,
					canMerge: result.permissions?.push ?? false,
				}),
			},
		);
	}
}

function injectEndpoints(api: BackendApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			getRepoInfo: build.query<RepoInfo, { projectId: string; owner: string; repo: string }>({
				extraOptions: { command: "get_repo_info" },
				query: (args) => args,
				providesTags: [providesList(ReduxTag.RepoInfo)],
			}),
		}),
	});
}

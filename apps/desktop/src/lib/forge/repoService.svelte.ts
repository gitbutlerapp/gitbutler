import { providesList, ReduxTag } from "$lib/state/tags";
import { InjectionToken } from "@gitbutler/core/context";
import type { BackendApi } from "$lib/state/backendApi";
import type { ReactiveQuery } from "$lib/state/butlerModule";
import type { RepoInfo } from "@gitbutler/but-sdk";

export const REPO_SERVICE = new InjectionToken<RepoService>("RepoService");

export interface RepoDetailedInfo {
	/** Whether the repository deletes the source branch after merge. */
	deleteBranchAfterMerge: boolean | undefined;
	/** Whether this repository is a fork. */
	fork: boolean;
	/** Caller's push permission (gates the merge button). */
	canMerge: boolean;
}

export class RepoService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(backendApi: BackendApi) {
		this.api = injectEndpoints(backendApi);
	}

	getInfo(projectId: string): ReactiveQuery<RepoDetailedInfo> {
		return this.api.endpoints.getRepoInfo.useQuery(
			{ projectId },
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
			getRepoInfo: build.query<RepoInfo, { projectId: string }>({
				extraOptions: { command: "get_repo_info" },
				query: (args) => args,
				providesTags: [providesList(ReduxTag.RepoInfo)],
			}),
		}),
	});
}

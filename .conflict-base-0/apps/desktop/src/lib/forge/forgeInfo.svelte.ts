import { providesType, ReduxTag } from "$lib/state/tags";
import { InjectionToken } from "@gitbutler/core/context";
import type { BackendApi } from "$lib/state/backendApi";
import type { ReactiveQuery } from "$lib/state/butlerModule";
import type { ForgeInfo } from "@gitbutler/but-sdk";

export const FORGE_INFO_SERVICE = new InjectionToken<ForgeInfoService>("ForgeInfoService");

export class ForgeInfoService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(backendApi: BackendApi) {
		this.api = injectEndpoints(backendApi);
	}

	get(projectId: string): ReactiveQuery<ForgeInfo | null> {
		return this.api.endpoints.forgeInfo.useQuery({ projectId });
	}

	compareBranchUrl(projectId: string, base: string, branch: string, fork: string | null) {
		return this.api.endpoints.forgeCompareBranchUrl.useQuery({
			projectId,
			base,
			branch,
			fork,
		});
	}

	async fetchCompareBranchUrl(
		projectId: string,
		base: string,
		branch: string,
		fork: string | null,
	) {
		return await this.api.endpoints.forgeCompareBranchUrl.fetch({
			projectId,
			base,
			branch,
			fork,
		});
	}
}

/** Append to `forgeInfo.baseUrl` patterns to build the various web URLs. */
export function commitUrl(forge: ForgeInfo, commitId: string): string {
	return `${forge.baseUrl}${forge.commitUrlPath}${commitId}`;
}

export function prUrl(forge: ForgeInfo, number: number): string {
	return `${forge.baseUrl}${forge.prUrlPath}${number}`;
}

function injectEndpoints(api: BackendApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			forgeInfo: build.query<ForgeInfo | null, { projectId: string }>({
				extraOptions: { command: "forge_info" },
				query: (args) => args,
				providesTags: [providesType(ReduxTag.ForgeProvider)],
			}),
			forgeCompareBranchUrl: build.query<
				string | null,
				{ projectId: string; base: string; branch: string; fork: string | null }
			>({
				extraOptions: { command: "forge_compare_branch_url" },
				query: (args) => args,
			}),
		}),
	});
}

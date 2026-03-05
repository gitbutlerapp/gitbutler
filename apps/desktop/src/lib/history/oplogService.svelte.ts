import { InjectionToken } from "@gitbutler/core/context";
import type { TreeChanges } from "$lib/hunks/change";
import type { BackendApi } from "$lib/state/clientState.svelte";

export const OPLOG_SERVICE = new InjectionToken<OplogService>("OplogService");

/** Supersedes the HistoryService */
export class OplogService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(backendApi: BackendApi) {
		this.api = injectEndpoints(backendApi);
	}

	get diffWorktree() {
		return this.api.endpoints.oplogDiffWorktrees.useQuery;
	}

	diffWorktreeByPath({
		projectId,
		snapshotId,
		path,
	}: {
		projectId: string;
		snapshotId: string;
		path: string;
	}) {
		return this.api.endpoints.oplogDiffWorktrees.useQuery(
			{ projectId, snapshotId },
			{
				transform: (result) => {
					return result.changes.find((change) => change.path === path);
				},
			},
		);
	}
}

function injectEndpoints(api: BackendApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			oplogDiffWorktrees: build.query<TreeChanges, { projectId: string; snapshotId: string }>({
				extraOptions: { command: "oplog_diff_worktrees" },
				query: (args) => args,
			}),
		}),
	});
}

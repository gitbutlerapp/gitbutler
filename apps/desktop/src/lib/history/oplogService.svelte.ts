import { InjectionToken } from "@gitbutler/core/context";
import type { BackendApi } from "$lib/state/clientState.svelte";

export const OPLOG_SERVICE = new InjectionToken<OplogService>("OplogService");

/** Supersedes the HistoryService */
export class OplogService {
	constructor(private backendApi: BackendApi) {}

	get diffWorktree() {
		return this.backendApi.endpoints.oplogDiffWorktrees.useQuery;
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
		return this.backendApi.endpoints.oplogDiffWorktrees.useQuery(
			{ projectId, snapshotId },
			{
				transform: (result) => {
					return result.changes.find((change) => change.path === path);
				},
			},
		);
	}
}

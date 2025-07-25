import { InjectionToken } from '@gitbutler/shared/context';
import type { TreeChanges } from '$lib/hunks/change';
import type { BackendApi, ClientState } from '$lib/state/clientState.svelte';

export const OPLOG_SERVICE = new InjectionToken<OplogService>('OplogService');

/** Supercedes the HistoryService */
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
		before,
		after,
		path
	}: {
		projectId: string;
		before: string;
		after: string;
		path: string;
	}) {
		return this.api.endpoints.oplogDiffWorktrees.useQuery(
			{ projectId, before, after },
			{
				transform: (result) => {
					return result.changes.find((change) => change.path === path);
				}
			}
		);
	}
}

function injectEndpoints(api: ClientState['backendApi']) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			oplogDiffWorktrees: build.query<
				TreeChanges,
				{ projectId: string; before: string; after: string }
			>({
				extraOptions: { command: 'oplog_diff_worktrees' },
				query: (args) => args
			})
		})
	});
}

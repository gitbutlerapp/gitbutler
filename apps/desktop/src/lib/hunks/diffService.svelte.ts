import { providesList, ReduxTag } from '$lib/state/tags';
import type { TreeChange } from '$lib/hunks/change';
import type { UnifiedDiff } from '$lib/hunks/diff';
import type { ClientState } from '$lib/state/clientState.svelte';

export type ChangeDiff = {
	path: string;
	diff: UnifiedDiff;
};

export class DiffService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(state: ClientState) {
		this.api = injectEndpoints(state.backendApi);
	}

	getDiff(projectId: string, change: TreeChange) {
		const { getDiff } = this.api.endpoints;
		const result = $derived(getDiff.useQuery({ projectId, change }));
		return result;
	}

	getChanges(projectId: string, changes: TreeChange[]) {
		const args = changes.map((change) => ({ projectId, change }));
		const { getDiff } = this.api.endpoints;
		return getDiff.useQueries(args, {
			transform: (data, args): ChangeDiff => ({ path: args.change.path, diff: data })
		});
	}
}
function injectEndpoints(api: ClientState['backendApi']) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			getDiff: build.query<UnifiedDiff, { projectId: string; change: TreeChange }>({
				query: ({ projectId, change }) => ({
					command: 'tree_change_diffs',
					params: { projectId, change }
				}),
				providesTags: [providesList(ReduxTag.Diff)]
			})
		})
	});
}

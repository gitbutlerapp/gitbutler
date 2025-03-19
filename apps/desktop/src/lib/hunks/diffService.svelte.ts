import { ReduxTag } from '$lib/state/tags';
import type { TreeChange } from '$lib/hunks/change';
import type { UnifiedDiff } from '$lib/hunks/diff';
import type { ClientState } from '$lib/state/clientState.svelte';

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
}

function injectEndpoints(api: ClientState['backendApi']) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			getDiff: build.query<UnifiedDiff, { projectId: string; change: TreeChange }>({
				query: ({ projectId, change }) => ({
					command: 'tree_change_diffs',
					params: { projectId, change }
				}),
				providesTags: [ReduxTag.Diff]
			})
		})
	});
}

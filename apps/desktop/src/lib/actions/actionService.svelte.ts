import { invalidatesList, ReduxTag } from '$lib/state/tags';
import type { TreeChange } from '$lib/hunks/change';
import type { BackendApi, ClientState } from '$lib/state/clientState.svelte';

export class ActionService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(backendApi: BackendApi) {
		this.api = injectEndpoints(backendApi);
	}

	get autoCommit() {
		return this.api.endpoints.autoCommit.useMutation();
	}

	get branchChanges() {
		return this.api.endpoints.autoBranchChanges.useMutation();
	}
}

function injectEndpoints(api: ClientState['backendApi']) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			autoCommit: build.mutation<void, { projectId: string; changes: TreeChange[] }>({
				query: ({ projectId, changes }) => ({
					command: 'auto_commit',
					params: { projectId, changes },
					actionName: 'Figure out where to commit the given changes'
				}),
				invalidatesTags: [
					invalidatesList(ReduxTag.Stacks),
					invalidatesList(ReduxTag.StackDetails),
					invalidatesList(ReduxTag.WorktreeChanges)
				]
			}),
			autoBranchChanges: build.mutation<void, { projectId: string; changes: TreeChange[] }>({
				query: ({ projectId, changes }) => ({
					command: 'auto_branch_changes',
					params: { projectId, changes },
					actionName: 'Create a branch for the given changes'
				}),
				invalidatesTags: [
					invalidatesList(ReduxTag.Stacks),
					invalidatesList(ReduxTag.StackDetails),
					invalidatesList(ReduxTag.WorktreeChanges)
				]
			})
		})
	});
}

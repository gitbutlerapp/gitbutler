import { hasTauriExtra } from '$lib/state/backendQuery';
import { ReduxTag } from '$lib/state/tags';
import type { WorktreeChanges } from '$lib/hunks/change';
import type { ClientState } from '$lib/state/clientState.svelte';

export class WorktreeService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(state: ClientState) {
		this.api = injectEndpoints(state.backendApi);
	}

	getChanges(projectId: string) {
		const { getWorktreeChanges } = this.api.endpoints;
		const result = $derived(getWorktreeChanges.useQuery({ projectId }));
		return result;
	}

	getChange(projectId: string, path: string) {
		const { getWorktreeChanges } = this.api.endpoints;
		const result = $derived(
			getWorktreeChanges.useQueryState({ projectId }, (result) => {
				return result.changes.find((change) => change.path === path);
			})
		);
		return result;
	}
}

function injectEndpoints(api: ClientState['backendApi']) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			getWorktreeChanges: build.query<WorktreeChanges, { projectId: string }>({
				query: ({ projectId }) => ({ command: 'worktree_changes', params: { projectId } }),
				providesTags: [ReduxTag.WorktreeChanges],
				// TODO: Customize this function to provide types for injected dependencies.
				async onCacheEntryAdded(arg, lifecycleApi) {
					if (!hasTauriExtra(lifecycleApi.extra)) {
						throw new Error('Redux dependency Tauri not found!');
					}
					await lifecycleApi.cacheDataLoaded;
					const unsubscribe = lifecycleApi.extra.tauri.listen<WorktreeChanges>(
						`project://${arg.projectId}/worktree_changes`,
						(event) => {
							lifecycleApi.updateCachedData(() => event.payload);
						}
					);
					await lifecycleApi.cacheEntryRemoved;
					unsubscribe();
				}
			})
		})
	});
}

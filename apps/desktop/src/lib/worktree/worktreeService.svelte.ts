import { hasTauriExtra } from '$lib/state/backendQuery';
import { ReduxTag } from '$lib/state/tags';
import { createEntityAdapter, type EntityState } from '@reduxjs/toolkit';
import type { TreeChange, WorktreeChanges } from '$lib/hunks/change';
import type { ClientState } from '$lib/state/clientState.svelte';

export class WorktreeService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(state: ClientState) {
		this.api = injectEndpoints(state.backendApi);
	}

	getChanges(projectId: string) {
		const { getChanges } = this.api.endpoints;
		const result = $derived(
			getChanges.useQuery({ projectId }, { transform: (res) => selectAll(res) })
		);
		return result;
	}

	getChange(projectId: string, path: string) {
		const { getChanges } = this.api.endpoints;
		const result = $derived(
			getChanges.useQueryState({ projectId }, { transform: (res) => selectById(res, path)! })
		);
		return result;
	}
}

function injectEndpoints(api: ClientState['backendApi']) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			getChanges: build.query<EntityState<TreeChange, string>, { projectId: string }>({
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
							lifecycleApi.dispatch(api.util.invalidateTags([ReduxTag.Diff]));
							lifecycleApi.updateCachedData(() => {
								console.log('streaming update', event);
								return worktreeAdapter.addMany(
									worktreeAdapter.getInitialState(),
									event.payload.changes
								);
							});
						}
					);
					await lifecycleApi.cacheEntryRemoved;
					unsubscribe();
				},
				/**
				 * For convenience we transform the result using the entity adapter such
				 * that we can use selectors like `selectById`.
				 */
				async transformResponse(response: WorktreeChanges) {
					return worktreeAdapter.addMany(worktreeAdapter.getInitialState(), response.changes);
				}
			})
		})
	});
}

const worktreeAdapter = createEntityAdapter<TreeChange, TreeChange['path']>({
	selectId: (change) => change.path,
	sortComparer: (a, b) => a.path.localeCompare(b.path)
});

const { selectAll, selectById } = worktreeAdapter.getSelectors();

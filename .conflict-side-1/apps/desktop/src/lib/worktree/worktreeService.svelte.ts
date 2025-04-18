import { hasTauriExtra } from '$lib/state/backendQuery';
import { createSelectByIds } from '$lib/state/customSelectors';
import { invalidatesList, providesList, ReduxTag } from '$lib/state/tags';
import { createEntityAdapter, type EntityState } from '@reduxjs/toolkit';
import type { TreeChange, WorktreeChanges } from '$lib/hunks/change';
import type { ClientState } from '$lib/state/clientState.svelte';

/**
 * A service for tracking uncommitted changes.
 *
 * Since we want to maintain a list and access individual records we use a
 * redux entity adapter on the results.
 */
export class WorktreeService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(state: ClientState) {
		this.api = injectEndpoints(state.backendApi);
	}

	/** Fetches and subscribes to a list of uncommitted changes. */
	getChanges(projectId: string) {
		const { getChanges } = this.api.endpoints;
		const result = $derived(getChanges.useQuery({ projectId }, { transform: selectAll }));
		return result;
	}

	/** Gets a specific change from any existing set of results. */
	getChange(projectId: string, path: string) {
		const { getChanges } = this.api.endpoints;
		const result = $derived(
			getChanges.useQueryState({ projectId }, { transform: (res) => selectById(res, path)! })
		);
		return result;
	}

	/** Gets a set of changes by the given paths */
	getChangesById(projectId: string, paths: string[]) {
		const { getChanges } = this.api.endpoints;
		const result = $derived(
			getChanges.useQueryState({ projectId }, { transform: (res) => selectByIds(res, paths) })
		);
		return result;
	}
}

function injectEndpoints(api: ClientState['backendApi']) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			/**
			 * Queries the backend for ucommitted changes.
			 *
			 * It is necessary to access to individual results by their id's, so we use a redux
			 * entity entity adapter to create the necessary selectors.
			 */
			getChanges: build.query<EntityState<TreeChange, string>, { projectId: string }>({
				query: ({ projectId }) => ({ command: 'changes_in_worktree', params: { projectId } }),
				/** Invalidating tags causes data to be refreshed. */
				providesTags: [providesList(ReduxTag.WorktreeChanges)],
				/**
				 * Sets up a subscription for changes to uncommitted changes until all consumers
				 * of the query results have unsubscribed.
				 */
				async onCacheEntryAdded(arg, lifecycleApi) {
					if (!hasTauriExtra(lifecycleApi.extra)) {
						throw new Error('Redux dependency Tauri not found!');
					}
					// The `cacheDataLoaded` promise resolves when the result is first loaded.
					await lifecycleApi.cacheDataLoaded;
					const unsubscribe = lifecycleApi.extra.tauri.listen<WorktreeChanges>(
						`project://${arg.projectId}/worktree_changes`,
						(event) => {
							lifecycleApi.dispatch(api.util.invalidateTags([invalidatesList(ReduxTag.Diff)]));
							lifecycleApi.updateCachedData(() =>
								worktreeAdapter.addMany(worktreeAdapter.getInitialState(), event.payload.changes)
							);
						}
					);
					// The `cacheEntryRemoved` promise resolves when the result is removed
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
const selectByIds = createSelectByIds<TreeChange>();

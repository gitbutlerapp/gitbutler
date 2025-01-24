import { invoke } from '$lib/backend/ipc';
import { hasTauriExtra, reduxApi } from '$lib/redux/api';
import { DesktopRedux } from '$lib/redux/store.svelte';
import { ReduxTag } from '$lib/redux/tags';
import { createSelector } from '@reduxjs/toolkit';
import type { WorktreeChanges, TreeChange, UnifiedDiff } from '$lib/hunks/change';

export class WorktreeService {
	private worktreeApi = reduxApi.injectEndpoints({
		endpoints: (build) => ({
			getWorktreeChanges: build.query<WorktreeChanges, { projectId: string }>({
				query: ({ projectId }) => ({ command: 'worktree_changes', params: { projectId } }),
				providesTags: [ReduxTag.WorktreeChanges],
				async onCacheEntryAdded(arg, lifecycleApi) {
					if (!hasTauriExtra(lifecycleApi.extra)) {
						throw new Error('Redux dependency Tauri not found!');
					}
					await lifecycleApi.cacheDataLoaded;
					const unsubscribe = lifecycleApi.extra.tauri.listen<WorktreeChanges>(
						`project://${arg.projectId}/worktree_changes`,
						(event) => {
							lifecycleApi.updateCachedData(() => {
								return event.payload;
							});
						}
					);
					await lifecycleApi.cacheEntryRemoved;
					unsubscribe();
				}
			})
		})
	});

	getChanges(projectId: string) {
		$effect(() => {
			const { unsubscribe } = this.state.dispatch(
				this.worktreeApi.endpoints.getWorktreeChanges.initiate({ projectId })
			);
			return () => {
				unsubscribe();
			};
		});
		return this.worktreeApi.endpoints.getWorktreeChanges.select({ projectId })(
			this.state.rootState$
		);
	}

	getChange(projectId: string, path: string) {
		return createSelector(
			(rootState: typeof this.state.rootState$) => rootState,
			(rootState) => {
				const { data } = this.worktreeApi.endpoints.getWorktreeChanges.select({ projectId })(
					rootState
				);
				if (data) {
					return data.changes.find((c) => c.path === path);
				}
			}
		)(this.state.rootState$);
	}

	constructor(private state: DesktopRedux) {}
}

/**
 * Gets the unified diff for a given TreeChange.
 * This probably does not belong in a package called "worktree" since this also operates on commit-to-commit changes and not only worktree changes
 */
export async function treeChangeDiffs(projectId: string, change: TreeChange) {
	return await invoke<UnifiedDiff>('tree_change_diffs', { projectId, change });
}

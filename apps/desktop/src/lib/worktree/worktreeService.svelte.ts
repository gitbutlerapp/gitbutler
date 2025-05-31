import { hasTauriExtra } from '$lib/state/backendQuery';
import { createSelectByIds } from '$lib/state/customSelectors';
import { invalidatesList, providesList, ReduxTag } from '$lib/state/tags';
import { createEntityAdapter, type EntityState } from '@reduxjs/toolkit';
import type { IgnoredChange, TreeChange, WorktreeChanges } from '$lib/hunks/change';
import type { HunkAssignments } from '$lib/hunks/diffService.svelte';
import type { HunkAssignment } from '$lib/hunks/hunk';
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

	changes(projectId: string) {
		const result = $derived(
			this.api.endpoints.worktreeChanges.useQuery(
				{ projectId },
				{ transform: (res) => worktreeSelectors.selectAll(res.changes) }
			)
		);
		return result;
	}

	assignments(projectId: string) {
		const result = $derived(
			this.api.endpoints.worktreeChanges.useQuery(
				{ projectId },
				{ transform: (res) => res.assignments }
			)
		);
		return result;
	}

	hunkAssignments(projectId: string) {
		const result = $derived(
			this.api.endpoints.worktreeChanges.useQuery(
				{ projectId },
				{ transform: (res) => res.hunkAssignments }
			)
		);
		return result;
	}

	worktreeData(projectId: string) {
		const result = $derived(this.api.endpoints.worktreeChanges.useQuery({ projectId }));
		return result;
	}

	getChange(projectId: string, path: string) {
		const { worktreeChanges: getChanges } = this.api.endpoints;
		return getChanges.useQueryState(
			{ projectId },
			{ transform: (res) => worktreeSelectors.selectById(res.changes, path)! }
		);
	}

	async fetchChange(projectId: string, path: string) {
		const { worktreeChanges: getChanges } = this.api.endpoints;
		return await getChanges.fetch(
			{ projectId },
			{ transform: (res) => worktreeSelectors.selectById(res.changes, path)! }
		);
	}

	/** Gets a set of changes by the given paths */
	getChangesById(projectId: string, paths: string[]) {
		const { worktreeChanges: getChanges } = this.api.endpoints;
		return getChanges.useQueryState(
			{ projectId },
			{ transform: (res) => worktreeSelectors.selectByIds(res.changes, paths) }
		);
	}

	/** Actively fetches the number of changes in the worktree */
	fetchChangesById(projectId: string, paths: string[]) {
		const { worktreeChanges: getChanges } = this.api.endpoints;
		return getChanges.fetch(
			{ projectId },
			{ transform: (res) => worktreeSelectors.selectByIds(res.changes, paths) }
		);
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
			worktreeChanges: build.query<
				{
					changes: EntityState<TreeChange, string>;
					rawChanges: TreeChange[];
					ignoredChanges: IgnoredChange[];
					assignments: HunkAssignments;
					hunkAssignments: HunkAssignment[];
				},
				{ projectId: string }
			>({
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
							lifecycleApi.updateCachedData((draft) => ({
								changes: worktreeAdapter.addMany(
									worktreeAdapter.getInitialState(),
									event.payload.changes
								),
								rawChanges: event.payload.changes,
								ignoredChanges: draft.ignoredChanges,
								assignments: groupAssignments(event.payload.assignments.Ok),
								hunkAssignments: event.payload.assignments.Ok
							}));
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
					return {
						changes: worktreeAdapter.addMany(worktreeAdapter.getInitialState(), response.changes),
						rawChanges: response.changes,
						ignoredChanges: response.ignoredChanges,
						assignments: groupAssignments(response.assignments.Ok),
						hunkAssignments: response.assignments.Ok
					};
				}
			})
		})
	});
}

const worktreeAdapter = createEntityAdapter<TreeChange, string>({
	selectId: (change) => change.path,
	sortComparer: (a, b) => a.path.localeCompare(b.path)
});

const worktreeSelectors = {
	...worktreeAdapter.getSelectors(),
	selectByIds: createSelectByIds<TreeChange>()
};

function groupAssignments(data: HunkAssignment[]): HunkAssignments {
	const groups: Record<string, Record<string, HunkAssignment[]>> = {};
	for (const assignment of data) {
		const key = assignment.stackId || 'unassigned';
		let stackGroup = groups[key];
		if (!stackGroup) {
			stackGroup = {};
			groups[key] = stackGroup;
		}
		let pathGroup = stackGroup[assignment.path];
		if (!pathGroup) {
			pathGroup = [];
			stackGroup[assignment.path] = pathGroup;
		}
		pathGroup.push(assignment);
	}
	return groups;
}

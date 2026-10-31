import { showError } from "$lib/error/showError";
import { shouldRaiseDependencyError, type DependencyError } from "$lib/hunks/dependencies";
import { shouldRaiseHunkAssignmentError } from "$lib/hunks/hunk";
import { checkoutOf } from "$lib/selection/key";
import { hasBackendExtra } from "$lib/state/backendQuery";
import { createSelectByIds } from "$lib/state/customSelectors";
import { invalidatesList, providesList, ReduxTag } from "$lib/state/tags";
import { createEntityAdapter, type EntityState } from "@reduxjs/toolkit";
import type { UnifiedDiff } from "$lib/hunks/diff";
import type { BackendEndpointBuilder } from "$lib/state/backendApi";
import type {
	HunkAssignment,
	HunkAssignmentRequest,
	HunkDependencies,
	IgnoredWorktreeChange,
	TreeChange,
	WorktreeChanges,
} from "@gitbutler/but-sdk";

export function buildWorktreeEndpoints(build: BackendEndpointBuilder) {
	return {
		// ── Worktree Changes ────────────────────────────────────────
		/** The uncommitted changes in the linked `worktree`, or in the main checkout. */
		worktreeChanges: build.query<WorktreeData, { projectId: string; worktree?: string }>({
			extraOptions: { command: "changes_in_worktree" },
			query: ({ projectId, worktree }) => ({
				projectId,
				changesSource: checkoutOf({ worktree }),
				computeDepsAndAssignments: true,
			}),
			providesTags: [providesList(ReduxTag.WorktreeChanges)],
			async onCacheEntryAdded(arg, lifecycleApi) {
				if (!hasBackendExtra(lifecycleApi.extra)) {
					throw new Error("Redux dependency Backend not found!");
				}
				await lifecycleApi.cacheDataLoaded;
				const { backend } = lifecycleApi.extra;
				function update(changes: WorktreeChanges) {
					lifecycleApi.updateCachedData(() => toWorktreeData(changes));
				}
				let unsubscribe: () => void;
				if (arg.worktree === undefined) {
					unsubscribe = backend.listen<WorktreeChanges>(
						`project://${arg.projectId}/worktree_changes`,
						(event) => update(event.payload),
					);
				} else {
					unsubscribe = backend.listen<{ worktree: string; changes: WorktreeChanges }>(
						`project://${arg.projectId}/linked_worktree_changes`,
						(event) => {
							if (event.payload.worktree === arg.worktree) update(event.payload.changes);
						},
					);
				}
				await lifecycleApi.cacheEntryRemoved;
				unsubscribe();
			},
			transformResponse(response: WorktreeChanges) {
				if (shouldRaiseDependencyError(response.dependenciesError)) {
					showError(
						"Failed to compute dependencies",
						response.dependenciesError.description,
						undefined,
						"worktree-dependencies-error",
					);
				}

				if (shouldRaiseHunkAssignmentError(response.assignmentsError)) {
					showError(
						"Failed to compute hunk assignments",
						response.assignmentsError.description,
						undefined,
						"worktree-assignments-error",
					);
				}

				return toWorktreeData(response);
			},
		}),

		// ── Diff ────────────────────────────────────────────────────
		/** An uncommitted change is read from the linked `worktree`'s files, or the main checkout's. */
		getDiff: build.query<
			UnifiedDiff | null,
			{ projectId: string; change: TreeChange; worktree?: string }
		>({
			extraOptions: { command: "tree_change_diffs_from_source" },
			query: ({ projectId, change, worktree }) => ({
				projectId,
				changesSource: checkoutOf({ worktree }),
				change,
			}),
			providesTags: [providesList(ReduxTag.Diff)],
		}),
		assignHunk: build.mutation<void, { projectId: string; assignments: HunkAssignmentRequest[] }>({
			extraOptions: {
				command: "assign_hunk",
			},
			query: (args) => args,
			invalidatesTags: [invalidatesList(ReduxTag.WorktreeChanges)],
		}),
		resolveWorktreeConflicts: build.mutation<void, { projectId: string; paths: string[] }>({
			extraOptions: {
				command: "resolve_worktree_conflicts",
				actionName: "Mark as Resolved",
			},
			query: (args) => args,
			invalidatesTags: [invalidatesList(ReduxTag.WorktreeChanges)],
		}),

		/**
		 * The absolute path of `path` in the linked `worktree`, or in the main
		 * checkout. A moved worktree refreshes the workspace view, and this with it.
		 */
		workdirPath: build.query<string, { projectId: string; worktree?: string; path: string }>({
			extraOptions: { command: "workdir_path" },
			query: ({ projectId, worktree, path }) => ({
				projectId,
				checkout: checkoutOf({ worktree }),
				path,
			}),
			providesTags: [providesList(ReduxTag.Stacks)],
		}),

		// ── File Search ─────────────────────────────────────────────
		findFiles: build.query<string[], { projectId: string; query: string; limit: number }>({
			extraOptions: { command: "find_files" },
			query: (args) => args,
			keepUnusedDataFor: 604800,
		}),
	};
}

const worktreeAdapter = createEntityAdapter<TreeChange, string>({
	selectId: (change) => change.path,
	sortComparer: (a, b) => a.path.localeCompare(b.path),
});

export const worktreeSelectors = {
	...worktreeAdapter.getSelectors(),
	selectByIds: createSelectByIds<TreeChange>(),
};

type WorktreeData = {
	changes: EntityState<TreeChange, string>;
	rawChanges: TreeChange[];
	ignoredChanges: IgnoredWorktreeChange[];
	hunkAssignments: HunkAssignment[];
	dependencies: HunkDependencies | undefined;
	dependenciesError: DependencyError | undefined;
};

function toWorktreeData(response: WorktreeChanges): WorktreeData {
	return {
		changes: worktreeAdapter.addMany(worktreeAdapter.getInitialState(), response.changes),
		rawChanges: response.changes,
		ignoredChanges: response.ignoredChanges,
		hunkAssignments: response.assignments,
		dependencies: response.dependencies ?? undefined,
		dependenciesError: response.dependenciesError ?? undefined,
	};
}

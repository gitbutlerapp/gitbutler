import { showError } from "$lib/error/showError";
import { shouldRaiseDependencyError, type DependencyError } from "$lib/hunks/dependencies";
import { shouldRaiseHunkAssignmentError } from "$lib/hunks/hunk";
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
		worktreeChanges: build.query<
			{
				changes: EntityState<TreeChange, string>;
				rawChanges: TreeChange[];
				ignoredChanges: IgnoredWorktreeChange[];
				hunkAssignments: HunkAssignment[];
				dependencies: HunkDependencies | undefined;
				dependenciesError: DependencyError | undefined;
			},
			{ projectId: string }
		>({
			extraOptions: { command: "changes_in_worktree" },
			query: (args) => args,
			providesTags: [providesList(ReduxTag.WorktreeChanges)],
			async onCacheEntryAdded(arg, lifecycleApi) {
				if (!hasBackendExtra(lifecycleApi.extra)) {
					throw new Error("Redux dependency Backend not found!");
				}
				await lifecycleApi.cacheDataLoaded;
				const unsubscribe = lifecycleApi.extra.backend.listen<WorktreeChanges>(
					`project://${arg.projectId}/worktree_changes`,
					(event) => {
						lifecycleApi.updateCachedData(() => ({
							changes: worktreeAdapter.addMany(
								worktreeAdapter.getInitialState(),
								event.payload.changes,
							),
							rawChanges: event.payload.changes,
							ignoredChanges: event.payload.ignoredChanges,
							hunkAssignments: event.payload.assignments,
							dependencies: event.payload.dependencies ?? undefined,
							dependenciesError: event.payload.dependenciesError ?? undefined,
						}));
					},
				);
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

				return {
					changes: worktreeAdapter.addMany(worktreeAdapter.getInitialState(), response.changes),
					rawChanges: response.changes,
					ignoredChanges: response.ignoredChanges,
					hunkAssignments: response.assignments,
					dependencies: response.dependencies ?? undefined,
					dependenciesError: response.dependenciesError ?? undefined,
				};
			},
		}),

		// ── Diff ────────────────────────────────────────────────────
		getDiff: build.query<UnifiedDiff | null, { projectId: string; change: TreeChange }>({
			extraOptions: { command: "tree_change_diffs" },
			query: (args) => args,
			providesTags: [providesList(ReduxTag.Diff)],
		}),
		assignHunk: build.mutation<void, { projectId: string; assignments: HunkAssignmentRequest[] }>({
			extraOptions: {
				command: "assign_hunk",
			},
			query: (args) => args,
			invalidatesTags: [invalidatesList(ReduxTag.WorktreeChanges)],
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

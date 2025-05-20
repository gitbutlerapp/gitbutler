import { providesList, ReduxTag } from '$lib/state/tags';
import type { TreeChange } from '$lib/hunks/change';
import type { UnifiedDiff } from '$lib/hunks/diff';
import type { AssignmentRejection, HunkAssignment, HunkAssignmentRequest } from '$lib/hunks/hunk';
import type { ClientState } from '$lib/state/clientState.svelte';
import type { WorktreeChangesKey } from '$lib/worktree/worktreeService.svelte';

export type ChangeDiff = {
	path: string;
	diff: UnifiedDiff;
};

export const ungroupedGroup = 'ungrouped';
export type HunkAssignments = Map<string, Map<string, HunkAssignment[]>>;
export type HunkGroup = { type: 'ungrouped' } | { type: 'grouped'; stackId: string };
export function hunkGroupToKey(a: HunkGroup): string {
	if (a.type === 'ungrouped') return ungroupedGroup;
	return a.stackId;
}
/**
 * Converts a hunk group key to a HunkGroup. This expects to be given a valid key.
 */
export function hunkGroupFromKey(a: string): HunkGroup {
	if (a === ungroupedGroup) return { type: 'ungrouped' };
	return {
		type: 'grouped',
		stackId: a
	};
}
export function hunkGroupEquals(a: HunkGroup, b: HunkGroup): boolean {
	if (a.type === 'ungrouped' && b.type === 'ungrouped') return true;
	if (a.type === 'grouped' && b.type === 'grouped' && a.stackId === b.stackId) return true;
	return false;
}

export class DiffService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(state: ClientState) {
		this.api = injectEndpoints(state.backendApi);
	}

	getDiff(projectId: string, change: TreeChange) {
		const { getDiff } = this.api.endpoints;
		return getDiff.useQuery({ projectId, change });
	}

	hunkAssignments(projectId: string, worktreeChangesKey: WorktreeChangesKey) {
		const { hunkAssignments } = this.api.endpoints;
		return hunkAssignments.useQuery(
			{ projectId, worktreeChangesKey },
			{
				transform: (data): HunkAssignments => {
					const groupedAssignments = new Map<string, Map<string, HunkAssignment[]>>();
					for (const assignment of data) {
						let stackGroup = groupedAssignments.get(assignment.stackId ?? ungroupedGroup);
						if (!stackGroup) {
							stackGroup = new Map();
							groupedAssignments.set(assignment.stackId ?? ungroupedGroup, stackGroup);
						}
						let pathGroup = stackGroup.get(assignment.path);
						if (!pathGroup) {
							pathGroup = [];
							stackGroup.set(assignment.path, pathGroup);
						}
						pathGroup.push(assignment);
					}
					return groupedAssignments;
				}
			}
		);
	}

	get assignHunk() {
		return this.api.endpoints.assignHunk.mutate;
	}

	async fetchDiff(projectId: string, change: TreeChange) {
		const { getDiff } = this.api.endpoints;
		return await getDiff.fetch({ projectId, change });
	}

	getChanges(projectId: string, changes: TreeChange[]) {
		const args = changes.map((change) => ({ projectId, change }));
		const { getDiff } = this.api.endpoints;
		return getDiff.useQueries(args, {
			transform: (data, args): ChangeDiff => ({ path: args.change.path, diff: data })
		});
	}

	async fetchChanges(projectId: string, changes: TreeChange[]): Promise<ChangeDiff[]> {
		const args = changes.map((change) => ({ projectId, change }));
		const responses = await Promise.all(
			args.map((arg) =>
				this.api.endpoints.getDiff.fetch(arg, {
					transform: (diff, args) => ({
						path: args.change.path,
						diff
					})
				})
			)
		);
		return responses.map((response) => response.data).filter((diff) => diff !== undefined);
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
			}),
			hunkAssignments: build.query<
				HunkAssignment[],
				{ projectId: string; worktreeChangesKey: WorktreeChangesKey }
			>({
				query: ({ projectId, worktreeChangesKey: _worktreeChangesKey }) => ({
					command: 'hunk_assignments',
					params: { projectId }
				}),
				providesTags: [providesList(ReduxTag.HunkAssignments)]
			}),
			assignHunk: build.mutation<
				AssignmentRejection[],
				{ projectId: string; assignments: HunkAssignmentRequest[] }
			>({
				query: ({ projectId, assignments }) => ({
					command: 'assign_hunk',
					params: { projectId, assignments }
				}),
				invalidatesTags: [providesList(ReduxTag.HunkAssignments)]
			})
		})
	});
}

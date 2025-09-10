import { invalidatesList, providesList, ReduxTag } from '$lib/state/tags';
import { InjectionToken } from '@gitbutler/core/context';
import { isDefined } from '@gitbutler/ui/utils/typeguards';
import type { TreeChange } from '$lib/hunks/change';
import type { UnifiedDiff } from '$lib/hunks/diff';
import type { AssignmentRejection, HunkAssignmentRequest } from '$lib/hunks/hunk';
import type { ClientState } from '$lib/state/clientState.svelte';

export type ChangeDiff = {
	path: string;
	diff: UnifiedDiff | null;
};

export const DIFF_SERVICE = new InjectionToken<DiffService>('DiffService');

export class DiffService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(state: ClientState) {
		this.api = injectEndpoints(state.backendApi);
	}

	getDiff(projectId: string, change: TreeChange) {
		return this.api.endpoints.getDiff.useQuery({ projectId, change });
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
		return responses.filter(isDefined);
	}
}
function injectEndpoints(api: ClientState['backendApi']) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			getDiff: build.query<UnifiedDiff | null, { projectId: string; change: TreeChange }>({
				extraOptions: { command: 'tree_change_diffs' },
				query: (args) => args,
				providesTags: [providesList(ReduxTag.Diff)]
			}),
			assignHunk: build.mutation<
				AssignmentRejection[],
				{ projectId: string; assignments: HunkAssignmentRequest[] }
			>({
				extraOptions: {
					command: 'assign_hunk'
				},
				query: (args) => args,
				invalidatesTags: [invalidatesList(ReduxTag.WorktreeChanges)]
			})
		})
	});
}

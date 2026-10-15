import { FILE_SELECTION_MANAGER } from "$lib/selection/fileSelectionManager.svelte";
import { UNCOMMITTED_SERVICE } from "$lib/selection/uncommittedService.svelte";
import { WORKTREE_SERVICE } from "$lib/worktree/worktreeService.svelte";
import { inject } from "@gitbutler/core/context";
import { untrack } from "svelte";

/**
 * Feeds the uncommitted changes of the linked `worktree`, or of the main
 * checkout, into its service, and drops selected files that are gone.
 */
export function syncUncommittedChanges(projectId: () => string, worktree?: string) {
	const worktreeService = inject(WORKTREE_SERVICE);
	const uncommittedService = inject(UNCOMMITTED_SERVICE);
	const idSelection = inject(FILE_SELECTION_MANAGER);

	const query = $derived(worktreeService.worktreeData(projectId(), worktree));
	const data = $derived(query.response);

	$effect(() => {
		if (!data) return;
		untrack(() => {
			uncommittedService
				.at(worktree)
				.updateData({ changes: data.rawChanges, assignments: data.hunkAssignments });
			idSelection.retain(
				data.rawChanges.map((change) => change.path),
				worktree,
			);
		});
	});
}

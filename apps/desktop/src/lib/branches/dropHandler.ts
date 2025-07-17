import { type BranchStack } from '$lib/branches/branch';
import { HunkDropData } from '$lib/dragging/draggables';
import { StackService } from '$lib/stacks/stackService.svelte';
import type { DropzoneHandler } from '$lib/dragging/handler';
/** Handler that moves uncommitted hunks between stacks. */
export class BranchHunkDzHandler implements DropzoneHandler {
	constructor(
		private stackService: StackService,
		private projectId: string,
		private stack: BranchStack
	) {}

	accepts(data: unknown) {
		return (
			data instanceof HunkDropData &&
			!data.commitId &&
			!data.hunk.locked &&
			data.branchId !== this.stack.id
		);
	}

	ondrop(data: HunkDropData) {
		const newOwnership = `${data.hunk.filePath}:${data.hunk.id}`;
		this.stackService.legacyUpdateBranchOwnership({
			projectId: this.projectId,
			stackId: this.stack.id,
			ownership: (newOwnership + '\n' + this.stack.ownership).trim()
		});
	}
}

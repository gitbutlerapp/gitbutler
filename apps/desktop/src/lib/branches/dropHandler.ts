import { type BranchStack } from '$lib/branches/branch';
import { filesToOwnership } from '$lib/branches/ownership';
import { HunkDropData, FileDropData } from '$lib/dragging/draggables';
import { LocalFile } from '$lib/files/file';
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

/** Handler that moves uncommitted files between stacks. */
export class BranchFileDzHandler implements DropzoneHandler {
	constructor(
		private stackService: StackService,
		private projectId: string,
		private stackId: string,
		private ownership: string
	) {}

	accepts(data: unknown) {
		return (
			data instanceof FileDropData &&
			data.file instanceof LocalFile &&
			this.stackId !== data.stackId &&
			!data.files.some((f) => f.locked)
		);
	}

	ondrop(data: FileDropData) {
		const newOwnership = filesToOwnership(data.files);
		this.stackService.legacyUpdateBranchOwnership({
			projectId: this.projectId,
			stackId: this.stackId,
			ownership: (newOwnership + '\n' + this.ownership).trim()
		});
	}
}

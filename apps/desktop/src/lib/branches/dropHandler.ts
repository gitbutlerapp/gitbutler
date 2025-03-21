import { type BranchStack } from '$lib/branches/branch';
import { filesToOwnership } from '$lib/branches/ownership';
import { HunkDropData, FileDropData } from '$lib/dragging/draggables';
import { LocalFile } from '$lib/files/file';
import type { BranchController } from '$lib/branches/branchController';
import type { DropzoneHandler } from '$lib/dragging/handler';

/** Handler that moves uncommitted hunks between stacks. */
export class BranchDzHandler implements DropzoneHandler {
	constructor(
		private branchController: BranchController,
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
		this.branchController.updateBranchOwnership(
			this.stack.id,
			(newOwnership + '\n' + this.stack.ownership).trim()
		);
	}
}

/** Handler that moves uncommitted files between stacks. */
export class BranchFileDzHandler implements DropzoneHandler {
	constructor(
		private branchController: BranchController,
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
		this.branchController.updateBranchOwnership(
			this.stackId,
			(newOwnership + '\n' + this.ownership).trim()
		);
	}
}

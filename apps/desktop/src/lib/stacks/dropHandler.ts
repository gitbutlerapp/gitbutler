import { filesToOwnership } from '$lib/branches/ownership';
import { FileDropData, HunkDropData } from '$lib/dragging/draggables';
import type { BranchController } from '$lib/branches/branchController';
import type { DropzoneHandler } from '$lib/dragging/handler';

/** Handler that creates a new stack from files or hunks. */
export class NewStackDzHandler implements DropzoneHandler {
	constructor(private branchController: BranchController) {}

	accepts(data: unknown) {
		if (data instanceof FileDropData) {
			return !(data.isCommitted || data.files.some((f) => f.locked));
		}
		if (data instanceof HunkDropData) {
			return !(data.isCommitted || data.hunk.locked);
		}
		return false;
	}

	ondrop(data: FileDropData | HunkDropData) {
		if (data instanceof HunkDropData) {
			const ownership = `${data.hunk.filePath}:${data.hunk.id}`;
			this.branchController.createBranch({ ownership });
		} else if (data instanceof FileDropData) {
			const ownership = filesToOwnership(data.files);
			this.branchController.createBranch({ ownership });
		}
	}
}

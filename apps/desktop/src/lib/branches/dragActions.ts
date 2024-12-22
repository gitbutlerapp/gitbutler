import { CommitDropData, HunkDropData, FileDropData } from '$lib/dragging/draggables';
import { filesToOwnership } from '$lib/vbranches/ownership';
import { LocalFile, type BranchStack } from '$lib/vbranches/types';
import type { BranchController } from '$lib/vbranches/branchController';

class BranchDragActions {
	constructor(
		private branchController: BranchController,
		private stack: BranchStack
	) {}

	acceptMoveCommit(data: any) {
		return (
			data instanceof CommitDropData && data.branchId !== this.stack.id && !data.commit.conflicted
		);
	}

	onMoveCommit(data: CommitDropData) {
		this.branchController.moveCommit(this.stack.id, data.commit.id, data.commit.branchId);
	}

	acceptBranchDrop(data: any) {
		if (data instanceof HunkDropData && !data.commitId && data.branchId !== this.stack.id) {
			return !data.hunk.locked;
		} else if (
			data instanceof FileDropData &&
			data.file instanceof LocalFile &&
			this.stack.id !== data.branchId
		) {
			return !data.files.some((f) => f.locked);
		} else {
			return false;
		}
	}

	onBranchDrop(data: HunkDropData | FileDropData) {
		if (data instanceof HunkDropData) {
			const newOwnership = `${data.hunk.filePath}:${data.hunk.id}`;
			this.branchController.updateBranchOwnership(
				this.stack.id,
				(newOwnership + '\n' + this.stack.ownership).trim()
			);
		} else if (data instanceof FileDropData) {
			const newOwnership = filesToOwnership(data.files);
			this.branchController.updateBranchOwnership(
				this.stack.id,
				(newOwnership + '\n' + this.stack.ownership).trim()
			);
		}
	}
}

export class BranchDragActionsFactory {
	constructor(private branchController: BranchController) {}

	build(stack: BranchStack) {
		return new BranchDragActions(this.branchController, stack);
	}
}

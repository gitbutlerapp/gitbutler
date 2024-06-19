import { DraggableCommit, DraggableHunk, DraggableFile } from '$lib/dragging/draggables';
import { filesToOwnership } from '$lib/vbranches/ownership';
import type { BranchController } from '$lib/vbranches/branchController';
import type { Branch } from '$lib/vbranches/types';

class BranchDragActions {
	constructor(
		private branchController: BranchController,
		private branch: Branch
	) {}

	acceptMoveCommit(data: any) {
		return data instanceof DraggableCommit && data.branchId !== this.branch.id && data.isHeadCommit;
	}

	onMoveCommit(data: DraggableCommit) {
		this.branchController.moveCommit(this.branch.id, data.commit.id);
	}

	acceptBranchDrop(data: any) {
		if (data instanceof DraggableHunk && data.branchId !== this.branch.id) {
			return !data.hunk.locked;
		} else if (data instanceof DraggableFile && data.branchId && data.branchId !== this.branch.id) {
			return !data.files.some((f) => f.locked);
		} else {
			return false;
		}
	}

	onBranchDrop(data: DraggableHunk | DraggableFile) {
		if (data instanceof DraggableHunk) {
			const newOwnership = `${data.hunk.filePath}:${data.hunk.id}`;
			this.branchController.updateBranchOwnership(
				this.branch.id,
				(newOwnership + '\n' + this.branch.ownership).trim()
			);
		} else if (data instanceof DraggableFile) {
			const newOwnership = filesToOwnership(data.files);
			this.branchController.updateBranchOwnership(
				this.branch.id,
				(newOwnership + '\n' + this.branch.ownership).trim()
			);
		}
	}
}

export class BranchDragActionsFactory {
	constructor(private branchController: BranchController) {}

	build(branch: Branch) {
		return new BranchDragActions(this.branchController, branch);
	}
}

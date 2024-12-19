import { DroppableCommit, DroppableHunk, DroppableFile } from '$lib/dragging/draggables';
import { filesToOwnership } from '$lib/vbranches/ownership';
import { LocalFile, type VirtualBranch } from '$lib/vbranches/types';
import type { BranchController } from '$lib/vbranches/branchController';

class BranchDragActions {
	constructor(
		private branchController: BranchController,
		private branch: VirtualBranch
	) {}

	acceptMoveCommit(data: any) {
		return (
			data instanceof DroppableCommit && data.branchId !== this.branch.id && !data.commit.conflicted
		);
	}

	onMoveCommit(data: DroppableCommit) {
		this.branchController.moveCommit(this.branch.id, data.commit.id, data.commit.branchId);
	}

	acceptBranchDrop(data: any) {
		if (data instanceof DroppableHunk && !data.commitId && data.branchId !== this.branch.id) {
			return !data.hunk.locked;
		} else if (
			data instanceof DroppableFile &&
			data.file instanceof LocalFile &&
			this.branch.id !== data.branchId
		) {
			return !data.files.some((f) => f.locked);
		} else {
			return false;
		}
	}

	onBranchDrop(data: DroppableHunk | DroppableFile) {
		if (data instanceof DroppableHunk) {
			const newOwnership = `${data.hunk.filePath}:${data.hunk.id}`;
			this.branchController.updateBranchOwnership(
				this.branch.id,
				(newOwnership + '\n' + this.branch.ownership).trim()
			);
		} else if (data instanceof DroppableFile) {
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

	build(branch: VirtualBranch) {
		return new BranchDragActions(this.branchController, branch);
	}
}

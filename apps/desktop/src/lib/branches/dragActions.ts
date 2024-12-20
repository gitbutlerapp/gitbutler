import { CommitDropData, HunkDropData, FileDropData } from '$lib/dragging/draggables';
import { filesToOwnership } from '$lib/vbranches/ownership';
import { LocalFile, type BranchStack } from '$lib/vbranches/types';
import type { BranchController } from '$lib/vbranches/branchController';

class BranchDragActions {
	constructor(
		private branchController: BranchController,
		private branch: BranchStack
	) {}

	acceptMoveCommit(data: any) {
		return (
			data instanceof CommitDropData && data.branchId !== this.branch.id && !data.commit.conflicted
		);
	}

	onMoveCommit(data: CommitDropData) {
		this.branchController.moveCommit(this.branch.id, data.commit.id, data.commit.branchId);
	}

	acceptBranchDrop(data: any) {
		if (data instanceof HunkDropData && !data.commitId && data.branchId !== this.branch.id) {
			return !data.hunk.locked;
		} else if (
			data instanceof FileDropData &&
			data.file instanceof LocalFile &&
			this.branch.id !== data.branchId
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
				this.branch.id,
				(newOwnership + '\n' + this.branch.ownership).trim()
			);
		} else if (data instanceof FileDropData) {
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

	build(branch: BranchStack) {
		return new BranchDragActions(this.branchController, branch);
	}
}

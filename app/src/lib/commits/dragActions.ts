import { DraggableCommit, DraggableFile, DraggableHunk } from '$lib/dragging/draggables';
import { filesToOwnership, filesToSimpleOwnership } from '$lib/vbranches/ownership';
import {
	LocalFile,
	RemoteCommit,
	RemoteFile,
	type Branch,
	type Commit
} from '$lib/vbranches/types';
import type { Project } from '$lib/backend/projects';
import type { BranchController } from '$lib/vbranches/branchController';

class CommitDragActions {
	constructor(
		private branchController: BranchController,
		private project: Project,
		private branch: Branch,
		private commit: Commit | RemoteCommit
	) {}

	acceptAmend(data: any) {
		if (this.commit instanceof RemoteCommit) {
			return false;
		}

		if (!this.project.ok_with_force_push && this.commit.isRemote) {
			return false;
		}

		if (this.commit.isIntegrated) {
			return false;
		}

		if (data instanceof DraggableHunk && data.branchId === this.branch.id) {
			return true;
		} else if (data instanceof DraggableFile && data.branchId === this.branch.id) {
			return true;
		} else {
			return false;
		}
	}

	onAmend(data: any) {
		if (data instanceof DraggableHunk) {
			const newOwnership = `${data.hunk.filePath}:${data.hunk.id}`;
			this.branchController.amendBranch(this.branch.id, this.commit.id, newOwnership);
		} else if (data instanceof DraggableFile) {
			if (data.file instanceof LocalFile) {
				// this is an uncommitted file change being amended to a previous commit
				const newOwnership = filesToOwnership(data.files);
				this.branchController.amendBranch(this.branch.id, this.commit.id, newOwnership);
			} else if (data.file instanceof RemoteFile) {
				// this is a file from a commit, rather than an uncommitted file
				const newOwnership = filesToSimpleOwnership(data.files);
				if (data.commit) {
					this.branchController.moveCommitFile(
						this.branch.id,
						data.commit.id,
						this.commit.id,
						newOwnership
					);
				}
			}
		}
	}

	acceptSquash(data: any) {
		if (this.commit instanceof RemoteCommit) {
			return false;
		}
		if (!(data instanceof DraggableCommit)) return false;
		if (data.branchId !== this.branch.id) return false;

		if (data.commit.isParentOf(this.commit)) {
			if (data.commit.isIntegrated) return false;
			if (data.commit.isRemote && !this.project.ok_with_force_push) return false;
			return true;
		} else if (this.commit.isParentOf(data.commit)) {
			if (this.commit.isIntegrated) return false;
			if (this.commit.isRemote && !this.project.ok_with_force_push) return false;
			return true;
		} else {
			return false;
		}
	}

	onSquash(data: any) {
		if (this.commit instanceof RemoteCommit) {
			return;
		}
		if (data.commit.isParentOf(this.commit)) {
			this.branchController.squashBranchCommit(data.branchId, this.commit.id);
		} else if (this.commit.isParentOf(data.commit)) {
			this.branchController.squashBranchCommit(data.branchId, data.commit.id);
		}
	}
}

export class CommitDragActionsFactory {
	constructor(
		private branchController: BranchController,
		private project: Project
	) {}

	build(branch: Branch, commit: Commit | RemoteCommit) {
		return new CommitDragActions(this.branchController, this.project, branch, commit);
	}
}

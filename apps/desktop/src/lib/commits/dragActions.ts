import { CommitDropData, FileDropData, HunkDropData } from '$lib/dragging/draggables';
import { filesToOwnership, filesToSimpleOwnership } from '$lib/vbranches/ownership';
import {
	LocalFile,
	Commit,
	RemoteFile,
	type VirtualBranch,
	type DetailedCommit
} from '$lib/vbranches/types';
import type { Project } from '$lib/backend/projects';
import type { BranchController } from '$lib/vbranches/branchController';

export class CommitDragActions {
	constructor(
		private branchController: BranchController,
		private project: Project,
		private branch: VirtualBranch,
		private commit: DetailedCommit | Commit
	) {}

	acceptsAmend(dropData: unknown): boolean {
		if (this.commit instanceof Commit) {
			return false;
		}

		if (!this.project.ok_with_force_push && this.commit.isRemote) {
			return false;
		}

		if (this.commit.isIntegrated) {
			return false;
		}

		if (
			dropData instanceof HunkDropData &&
			dropData.branchId === this.branch.id &&
			dropData.commitId !== this.commit.id &&
			!this.commit.conflicted
		) {
			return true;
		} else if (
			dropData instanceof FileDropData &&
			dropData.branchId === this.branch.id &&
			dropData.commit?.id !== this.commit.id &&
			!this.commit.conflicted
		) {
			return true;
		} else {
			return false;
		}
	}

	onAmend(dropData: unknown): void {
		if (dropData instanceof HunkDropData) {
			const newOwnership = `${dropData.hunk.filePath}:${dropData.hunk.id}`;
			this.branchController.amendBranch(this.branch.id, this.commit.id, newOwnership);
		} else if (dropData instanceof FileDropData) {
			if (dropData.file instanceof LocalFile) {
				// this is an uncommitted file change being amended to a previous commit
				const newOwnership = filesToOwnership(dropData.files);
				this.branchController.amendBranch(this.branch.id, this.commit.id, newOwnership);
			} else if (dropData.file instanceof RemoteFile) {
				// this is a file from a commit, rather than an uncommitted file
				const newOwnership = filesToSimpleOwnership(dropData.files);
				if (dropData.commit) {
					this.branchController.moveCommitFile(
						this.branch.id,
						dropData.commit.id,
						this.commit.id,
						newOwnership
					);
				}
			}
		}
	}

	acceptsSquash(dropData: unknown): boolean {
		if (this.commit instanceof Commit) {
			return false;
		}
		if (!(dropData instanceof CommitDropData)) return false;
		if (dropData.branchId !== this.branch.id) return false;

		if (this.commit.conflicted || dropData.commit.conflicted) return false;

		if (dropData.commit.isParentOf(this.commit)) {
			if (dropData.commit.isIntegrated) return false;
			if (dropData.commit.isRemote && !this.project.ok_with_force_push) return false;
			return true;
		} else if (this.commit.isParentOf(dropData.commit)) {
			if (this.commit.isIntegrated) return false;
			if (this.commit.isRemote && !this.project.ok_with_force_push) return false;
			return true;
		} else {
			return false;
		}
	}

	onSquash(dropData: unknown): void {
		if (this.commit instanceof Commit) {
			return;
		}
		if (dropData instanceof CommitDropData) {
			if (dropData.commit.isParentOf(this.commit)) {
				this.branchController.squashBranchCommit(dropData.branchId, this.commit.id);
			} else if (this.commit.isParentOf(dropData.commit)) {
				this.branchController.squashBranchCommit(dropData.branchId, dropData.commit.id);
			}
		}
	}
}

export class CommitDragActionsFactory {
	constructor(
		private branchController: BranchController,
		private project: Project
	) {}

	build(branch: VirtualBranch, commit: DetailedCommit | Commit) {
		return new CommitDragActions(this.branchController, this.project, branch, commit);
	}
}

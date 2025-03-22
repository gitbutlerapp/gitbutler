import { type BranchStack } from '$lib/branches/branch';
import { filesToSimpleOwnership } from '$lib/branches/ownership';
import { ChangeDropData, FileDropData, HunkDropData } from '$lib/dragging/draggables';
import { LocalFile, RemoteFile } from '$lib/files/file';
import type { BranchController } from '$lib/branches/branchController';
import type { DropzoneHandler } from '$lib/dragging/handler';

/** Details about a commit beloning to a drop zone. */
export type DzCommitData = {
	id: string;
	isRemote: boolean;
	isIntegrated: boolean;
	isConflicted: boolean;
};

/** Details about a commit that can be dropped into a drop zone. */
export class CommitDropData {
	constructor(
		readonly stackId: string,
		readonly commit: DzCommitData,
		readonly isHeadCommit: boolean,
		readonly branchName?: string
	) {}
}

/** Handler that can move commits between stacks. */
export class MoveCommitDzHandler implements DropzoneHandler {
	constructor(
		private branchController: BranchController,
		private stack: BranchStack
	) {}

	accepts(data: unknown): boolean {
		return (
			data instanceof CommitDropData && data.stackId !== this.stack.id && !data.commit.isConflicted
		);
	}
	ondrop(data: CommitDropData): void {
		this.branchController.moveCommit(this.stack.id, data.commit.id, data.stackId);
	}
}

/**
 * Handler that will be able to amend a commit using `TreeChange`.
 */
export class AmendCommitWithChangeDzHandler implements DropzoneHandler {
	constructor(
		private stackId: string,
		private commit: DzCommitData
	) {}
	accepts(data: unknown): boolean {
		return (
			data instanceof ChangeDropData && data.stackId !== this.stackId && !this.commit.isConflicted
		);
	}
	ondrop(_data: ChangeDropData): void {
		throw new Error('Method not implemented.');
	}
}

/**
 * Handler that is able to amend a commit using `Hunk`.
 *
 * TODO: Refactor this to be V2 & V3 compatible.
 */
export class AmendCommitWithHunkDzHandler implements DropzoneHandler {
	constructor(
		private args: {
			branchController: BranchController;
			okWithForce: boolean;
			stackId: string;
			commit: DzCommitData;
		}
	) {}

	accepts(data: unknown): boolean {
		const { stackId, commit, okWithForce } = this.args;
		if (!okWithForce && commit.isRemote) return false;
		if (commit.isIntegrated) return false;
		return (
			data instanceof HunkDropData &&
			data.branchId === stackId &&
			data.commitId !== commit.id &&
			!commit.isConflicted
		);
	}

	ondrop(data: HunkDropData): void {
		const { branchController, stackId, commit, okWithForce } = this.args;
		if (!okWithForce && commit.isRemote) return;
		branchController.amendBranch(stackId, commit.id, [
			{
				// TODO: We need the previous path bytes added here.
				previousPathBytes: null,
				// TODO: We need to change this to path bytes.
				pathBytes: data.hunk.filePath,
				hunkHeaders: [
					{
						oldStart: data.hunk.oldStart,
						oldLines: data.hunk.oldLines,
						newStart: data.hunk.newStart,
						newLines: data.hunk.newLines
					}
				]
			}
		]);
	}
}

/**
 * Handler that is able to amend a commit using `AnyFile`.
 *
 * TODO: Refactor this to be V2 & V3 compatible.
 */
export class AmendCommitDzHandler implements DropzoneHandler {
	constructor(
		private args: {
			branchController: BranchController;
			okWithForce: boolean;
			stackId: string;
			commit: DzCommitData;
		}
	) {}

	accepts(dropData: unknown): boolean {
		const { stackId, commit, okWithForce } = this.args;
		if (!okWithForce && commit.isRemote) return false;
		if (commit.isIntegrated) return false;
		return (
			dropData instanceof FileDropData &&
			dropData.stackId === stackId &&
			dropData.commit?.id !== commit.id &&
			!commit.isConflicted
		);
	}

	ondrop(data: FileDropData): void {
		const { branchController, stackId, commit } = this.args;
		if (data.file instanceof LocalFile) {
			const worktreeChanges = data.files.map((file) => {
				return {
					previousPathBytes: null,
					pathBytes: file.path, // Can we get the path in bytes here?
					hunkHeaders: [] // An empty list of hunk headers means use everything for the file
				};
			});
			branchController.amendBranch(stackId, commit.id, worktreeChanges);
		} else if (data.file instanceof RemoteFile) {
			// this is a file from a commit, rather than an uncommitted file
			const newOwnership = filesToSimpleOwnership(data.files);
			if (data.commit) {
				branchController.moveCommitFile(stackId, data.commit.id, commit.id, newOwnership);
			}
		}
	}
}

/**
 * Handler that is able to squash two commits using `DzCommitData`.
 */
export class SquashCommitDzHandler implements DropzoneHandler {
	constructor(
		private args: {
			branchController: BranchController;
			stackId: string;
			commit: DzCommitData;
		}
	) {}

	accepts(data: unknown): boolean {
		const { stackId, commit } = this.args;
		if (!(data instanceof CommitDropData)) return false;
		if (data.stackId !== stackId) return false;

		if (commit.isConflicted || data.commit.isConflicted) return false;
		if (commit.id === data.commit.id) return false;

		return true;
	}

	ondrop(data: unknown): void {
		const { branchController, commit } = this.args;
		if (data instanceof CommitDropData) {
			branchController.squashBranchCommit(data.stackId, data.commit.id, commit.id);
		}
	}
}

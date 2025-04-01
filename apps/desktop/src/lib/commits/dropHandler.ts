import { type BranchStack } from '$lib/branches/branch';
import { filesToSimpleOwnership } from '$lib/branches/ownership';
import { ChangeDropData, FileDropData, HunkDropData } from '$lib/dragging/draggables';
import { LocalFile, RemoteFile } from '$lib/files/file';
import type { BranchController } from '$lib/branches/branchController';
import type { DropzoneHandler } from '$lib/dragging/handler';
import type { DiffSpec } from '$lib/hunks/hunk';
import type { StackService } from '$lib/stacks/stackService.svelte';

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
	trigger: ReturnType<StackService['amendCommit']>[0];
	result: ReturnType<StackService['amendCommit']>[1];

	constructor(
		private projectId: string,
		stackService: StackService,
		private stackId: string,
		private commit: DzCommitData,
		private onresult: (result: typeof this.result.current.data) => void
	) {
		const [trigger, result] = stackService.amendCommit();
		this.trigger = trigger;
		this.result = result;
	}
	accepts(data: unknown): boolean {
		return (
			data instanceof ChangeDropData && data.stackId !== this.stackId && !this.commit.isConflicted
		);
	}

	async ondrop(data: ChangeDropData) {
		const result = await this.trigger({
			projectId: this.projectId,
			stackId: this.stackId,
			commitId: this.commit.id,
			worktreeChanges: changesToDiffSpec(data)
		});

		this.onresult(result.data);
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
			branchController.amendBranch(stackId, commit.id, filesToDiffSpec(data));
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

/** Helper function that converts `FileDropData` to `DiffSpec`. */
function filesToDiffSpec(data: FileDropData): DiffSpec[] {
	return data.files.map((file) => {
		return {
			previousPathBytes: null,
			pathBytes: file.path, // Can we get the path in bytes here?
			hunkHeaders: []
		};
	});
}

/** Helper function that converts `ChangeDropData` to `DiffSpec`. */
function changesToDiffSpec(data: ChangeDropData): DiffSpec[] {
	const file = data.file;
	return [
		{
			previousPathBytes: null,
			pathBytes: file.path, // Can we get the path in bytes here?
			hunkHeaders: []
		}
	];
}

import { type BranchStack } from '$lib/branches/branch';
import { filesToSimpleOwnership } from '$lib/branches/ownership';
import { ChangeDropData, FileDropData, HunkDropData } from '$lib/dragging/draggables';
import { LocalFile, RemoteFile } from '$lib/files/file';
import type { DropzoneHandler } from '$lib/dragging/handler';
import type { DiffSpec } from '$lib/hunks/hunk';
import type { StackService } from '$lib/stacks/stackService.svelte';

/** Details about a commit beloning to a drop zone. */
export type DzCommitData = {
	id: string;
	isRemote: boolean;
	isIntegrated: boolean;
	hasConflicts: boolean;
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
		private stackService: StackService,
		private stack: BranchStack,
		private projectId: string
	) {}

	accepts(data: unknown): boolean {
		return (
			data instanceof CommitDropData && data.stackId !== this.stack.id && !data.commit.hasConflicts
		);
	}
	ondrop(data: CommitDropData): void {
		this.stackService.moveCommit({
			projectId: this.projectId,
			targetStackId: this.stack.id,
			commitOid: data.commit.id,
			sourceStackId: data.stackId
		});
	}
}

/**
 * Handler that will be able to amend a commit using `TreeChange`.
 */
export class AmendCommitWithChangeDzHandler implements DropzoneHandler {
	trigger: StackService['amendCommit'][0];
	result: StackService['amendCommit'][1];

	constructor(
		private projectId: string,
		stackService: StackService,
		private stackId: string,
		private commit: DzCommitData,
		private onresult: (result: typeof this.result.current.data) => void
	) {
		const [trigger, result] = stackService.amendCommit;
		this.trigger = trigger;
		this.result = result;
	}
	accepts(data: unknown): boolean {
		return data instanceof ChangeDropData && !this.commit.hasConflicts;
	}

	async ondrop(data: ChangeDropData) {
		this.onresult(
			await this.trigger({
				projectId: this.projectId,
				stackId: this.stackId,
				commitId: this.commit.id,
				worktreeChanges: changesToDiffSpec(data)
			})
		);
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
			stackService: StackService;
			okWithForce: boolean;
			projectId: string;
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
			!commit.hasConflicts
		);
	}

	ondrop(data: HunkDropData): void {
		const { stackService, projectId, stackId, commit, okWithForce } = this.args;
		if (!okWithForce && commit.isRemote) return;
		stackService.amendCommitMutation({
			projectId,
			stackId,
			commitId: commit.id,
			worktreeChanges: [
				{
					// TODO: We don't get prev path bytes in v2, but we're using
					// the new api.
					previousPathBytes: null,
					pathBytes: data.hunk.filePath as any,
					hunkHeaders: [
						{
							oldStart: data.hunk.oldStart,
							oldLines: data.hunk.oldLines,
							newStart: data.hunk.newStart,
							newLines: data.hunk.newLines
						}
					]
				}
			]
		});
	}
}

/**
 * Handler that is able to amend a commit using `AnyFile`.
 */
export class AmendCommitDzHandler implements DropzoneHandler {
	constructor(
		private args: {
			stackService: StackService;
			okWithForce: boolean;
			projectId: string;
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
			!commit.hasConflicts
		);
	}

	ondrop(data: FileDropData): void {
		const { stackService, projectId, stackId, commit } = this.args;
		if (data.file instanceof LocalFile) {
			stackService.amendCommitMutation({
				projectId,
				stackId,
				commitId: commit.id,
				worktreeChanges: filesToDiffSpec(data)
			});
		} else if (data.file instanceof RemoteFile) {
			// this is a file from a commit, rather than an uncommitted file
			const newOwnership = filesToSimpleOwnership(data.files);
			if (data.commit) {
				stackService.moveCommitFileMutation({
					projectId,
					stackId,
					fromCommitOid: data.commit.id,
					toCommitOid: commit.id,
					ownership: newOwnership
				});
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
			stackService: StackService;
			projectId: string;
			stackId: string;
			commit: DzCommitData;
		}
	) {}

	accepts(data: unknown): boolean {
		const { stackId, commit } = this.args;
		if (!(data instanceof CommitDropData)) return false;
		if (data.stackId !== stackId) return false;

		if (commit.hasConflicts || data.commit.hasConflicts) return false;
		if (commit.id === data.commit.id) return false;

		return true;
	}

	async ondrop(data: unknown) {
		const { stackService, projectId, stackId, commit } = this.args;
		if (data instanceof CommitDropData) {
			await stackService.squashCommits({
				projectId,
				stackId,
				sourceCommitOids: [data.commit.id],
				targetCommitOid: commit.id
			});
		}
	}
}

/** Helper function that converts `FileDropData` to `DiffSpec`. */
function filesToDiffSpec(data: FileDropData): DiffSpec[] {
	return data.files.map((file) => {
		return {
			previousPathBytes: null,
			pathBytes: file.path as any, // Rust type is BString.
			hunkHeaders: []
		};
	});
}

/** Helper function that converts `ChangeDropData` to `DiffSpec`. */
function changesToDiffSpec(data: ChangeDropData): DiffSpec[] {
	const filePaths = data.filePaths;
	return filePaths.map((filePath) => {
		return {
			previousPathBytes: null,
			pathBytes: filePath as any, // Rust type is Bstring.
			hunkHeaders: []
		};
	});
}

import { filesToSimpleOwnership } from '$lib/branches/ownership';
import {
	ChangeDropData,
	FileDropData,
	HunkDropData,
	HunkDropDataV3
} from '$lib/dragging/draggables';
import { LocalFile, RemoteFile } from '$lib/files/file';
import type { DropzoneHandler } from '$lib/dragging/handler';
import type { DiffSpec } from '$lib/hunks/hunk';
import type { ChangeSelectionService } from '$lib/selection/changeSelection.svelte';
import type { StackService } from '$lib/stacks/stackService.svelte';
import type { UiState } from '$lib/state/uiState.svelte';

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

export class StartCommitDzHandler implements DropzoneHandler {
	constructor(
		private args: {
			uiState: UiState;
			changeSelectionService: ChangeSelectionService;
			stackId: string;
			projectId: string;
			branchName: string;
		}
	) {}

	accepts(data: unknown): boolean {
		return (data instanceof ChangeDropData && !data.isCommitted) || data instanceof HunkDropDataV3;
	}
	ondrop(data: ChangeDropData | HunkDropDataV3): void {
		const { projectId, stackId, branchName, uiState, changeSelectionService } = this.args;

		const projectState = uiState.project(projectId);
		const stackState = stackId ? uiState.stack(stackId) : undefined;

		if (data instanceof ChangeDropData) {
			changeSelectionService.upsert({
				type: 'full',
				path: data.change.path,
				pathBytes: data.change.pathBytes,
				previousPathBytes:
					data.change.status.type === 'Rename' ? data.change.status.subject.previousPathBytes : null
			});
		} else if (data instanceof HunkDropDataV3) {
			const fileSelection = changeSelectionService.getById(data.change.path).current;
			const hunks = fileSelection?.type === 'partial' ? fileSelection.hunks.slice() : [];
			hunks.push({ ...data.hunk, type: 'full' });
			changeSelectionService.upsert({
				type: 'partial',
				path: data.change.path,
				pathBytes: data.change.pathBytes,
				previousPathBytes:
					data.change.status.type === 'Rename'
						? data.change.status.subject.previousPathBytes
						: null,
				hunks
			});
		}

		projectState.drawerPage.set('new-commit');
		projectState.stackId.set(stackId);
		stackState?.selection.set({ branchName: branchName });
	}
}

/** Handler that can move commits between stacks. */
export class MoveCommitDzHandler implements DropzoneHandler {
	constructor(
		private stackService: StackService,
		private stackId: string,
		private projectId: string
	) {}

	accepts(data: unknown): boolean {
		return (
			data instanceof CommitDropData && data.stackId !== this.stackId && !data.commit.hasConflicts
		);
	}
	ondrop(data: CommitDropData): void {
		this.stackService.moveCommit({
			projectId: this.projectId,
			targetStackId: this.stackId,
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
		switch (data.selectionId.type) {
			case 'commit':
			case 'branch':
				console.warn('Moving a change from one commit to another is not supported yet.');
				break;
			case 'worktree':
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

	private acceptsHunkV2(data: unknown): boolean {
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

	private acceptsHunkV3(data: unknown): boolean {
		const { commit, okWithForce } = this.args;
		if (!okWithForce && commit.isRemote) return false;
		if (commit.isIntegrated) return false;
		return data instanceof HunkDropDataV3 && !commit.hasConflicts;
	}

	accepts(data: unknown): boolean {
		return this.acceptsHunkV2(data) || this.acceptsHunkV3(data);
	}

	ondrop(data: HunkDropData | HunkDropDataV3): void {
		const { stackService, projectId, stackId, commit, okWithForce } = this.args;
		if (!okWithForce && commit.isRemote) return;

		if (data instanceof HunkDropData) {
			if (data.isCommitted) {
				// TODO: Move a hunk from one commit to another in v2
				console.warn('Moving a hunk from one commit to another is not supported yet.');
				return;
			}

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
			return;
		}

		if (data instanceof HunkDropDataV3) {
			const previousPathBytes =
				data.change.status.type === 'Rename' ? data.change.status.subject.previousPathBytes : null;

			if (!data.uncommitted) {
				// TODO: Move a hunk from one commit to another.
				console.warn('Moving a hunk from one commit to another is not supported yet.');
				return;
			}

			stackService.amendCommitMutation({
				projectId,
				stackId,
				commitId: commit.id,
				worktreeChanges: [
					{
						previousPathBytes,
						pathBytes: data.change.pathBytes,
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
			return;
		}
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

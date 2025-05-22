import { filesToOwnership } from '$lib/branches/ownership';
import { changesToDiffSpec } from '$lib/commits/utils';
import { ChangeDropData, FileDropData, HunkDropData } from '$lib/dragging/draggables';
import StackMacros from '$lib/stacks/macros';
import type { DropzoneHandler } from '$lib/dragging/handler';
import type {
	CreateCommitRequestWorktreeChanges,
	StackService
} from '$lib/stacks/stackService.svelte';
import type { UiState } from '$lib/state/uiState.svelte';

/** Handler that creates a new stack from files or hunks. */
export class NewStackDzHandler implements DropzoneHandler {
	constructor(
		private stackService: StackService,
		private projectId: string
	) {}

	accepts(data: unknown) {
		if (data instanceof FileDropData) {
			return !(data.isCommitted || data.files.some((f) => f.locked));
		}
		if (data instanceof HunkDropData) {
			return !(data.isCommitted || data.hunk.locked);
		}
		return false;
	}

	ondrop(data: FileDropData | HunkDropData) {
		if (data instanceof HunkDropData) {
			const ownership = `${data.hunk.filePath}:${data.hunk.id}`;
			this.stackService.newStackMutation({ projectId: this.projectId, branch: { ownership } });
		} else if (data instanceof FileDropData) {
			const ownership = filesToOwnership(data.files);
			this.stackService.newStackMutation({ projectId: this.projectId, branch: { ownership } });
		}
	}
}

/** Handler when drop changes on a special outside lanes dropzone. */
export class OutsideLaneDzHandler implements DropzoneHandler {
	private macros: StackMacros;

	constructor(
		private stackService: StackService,
		private projectId: string,
		private readonly uiState: UiState
	) {
		this.macros = new StackMacros(this.projectId, this.stackService, this.uiState);
	}

	accepts(data: unknown) {
		if (!(data instanceof ChangeDropData)) return false;
		if (data.selectionId.type === 'branch') return false;
		if (data.selectionId.type === 'commit' && data.stackId === undefined) return false;
		return true;
	}

	async ondrop(data: ChangeDropData) {
		switch (data.selectionId.type) {
			case 'branch': {
				// This should never happen, but just in case
				console.warn('Moving changes from a branch to a new stack is not supported');
				break;
			}
			case 'commit': {
				const { stack, outcome, branchName } = await this.macros.createNewStackAndCommit();

				const sourceStackId = data.stackId;
				const sourceCommitId = data.selectionId.commitId;
				if (sourceStackId) {
					await this.macros.moveChangesToNewCommit(
						stack.id,
						outcome.newCommit,
						sourceStackId,
						sourceCommitId,
						branchName,
						changesToDiffSpec(data)
					);
				} else {
					// Should not happen, but just in case
					throw new Error('Change drop data must specify the source stackId');
				}
				break;
			}
			case 'worktree': {
				const worktreeChanges = changesToWorktreeChanges(data);
				this.macros.branchChanges({ worktreeChanges });
				break;
			}
		}
	}
}

/**
 * Converts a `ChangeDropData` object into an array of `CreateCommitRequestWorktreeChanges`.
 */
function changesToWorktreeChanges(changes: ChangeDropData): CreateCommitRequestWorktreeChanges[] {
	const worktreeChanges: CreateCommitRequestWorktreeChanges[] = [];
	for (const change of changes.changes) {
		const previousPathBytes =
			change.status.type === 'Rename' ? change.status.subject.previousPathBytes : null;
		worktreeChanges.push({
			pathBytes: change.pathBytes,
			previousPathBytes,
			hunkHeaders: []
		});
	}

	return worktreeChanges;
}

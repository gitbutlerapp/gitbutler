import { filesToOwnership } from '$lib/branches/ownership';
import { changesToDiffSpec } from '$lib/commits/utils';
import { ChangeDropData, FileDropData, HunkDropData } from '$lib/dragging/draggables';
import { showError } from '$lib/notifications/toasts';
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

const STUB_COMMIT_MESSAGE = 'New commit';
/** Handler when drop changes on a special outside lanes dropzone. */
export class OutsideLaneDzHandler implements DropzoneHandler {
	constructor(
		private stackService: StackService,
		private projectId: string,
		private readonly uiState: UiState
	) {}

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
				const { stack, outcome, branchName } = await this.createNewStackAndCommit();

				const sourceStackId = data.stackId;
				const sourceCommitId = data.selectionId.commitId;
				if (sourceStackId) {
					await this.moveChangesToNewCommit(
						stack.id,
						outcome.newCommit,
						sourceStackId,
						sourceCommitId,
						branchName,
						data
					);
				} else {
					// Should not happen, but just in case
					throw new Error('Change drop data must specify the source stackId');
				}
				break;
			}
			case 'worktree': {
				const worktreeChanges = changesToWorktreeChanges(data);
				const { stack, outcome, branchName } = await this.createNewStackAndCommit(worktreeChanges);

				this.uiState.stack(stack.id).selection.set({
					branchName,
					commitId: outcome.newCommit
				});

				break;
			}
		}
	}

	/**
	 * Moves the changes from the source commit to the new commit in the new stack.
	 */
	private async moveChangesToNewCommit(
		destinationStackId: string,
		destinationCommitId: string,
		sourceStackId: string,
		sourceCommitId: string,
		branchName: string,
		data: ChangeDropData
	) {
		const { replacedCommits } = await this.stackService.moveChangesBetweenCommits({
			projectId: this.projectId,
			destinationStackId: destinationStackId,
			destinationCommitId: destinationCommitId,
			sourceStackId,
			sourceCommitId,
			changes: changesToDiffSpec(data)
		});

		const newCommitId = replacedCommits.find(([before]) => before === destinationCommitId)?.[1];
		if (!newCommitId) {
			// This happend only if something went wrong
			throw new Error('No new commit id found for the moved changes');
		}

		this.uiState.stack(destinationStackId).selection.set({
			branchName,
			commitId: newCommitId
		});
		this.uiState.project(this.projectId).stackId.set(destinationStackId);
	}

	/**
	 * Creates a new stack and stub commit into it.
	 */
	private async createNewStackAndCommit(
		worktreeChanges: CreateCommitRequestWorktreeChanges[] = []
	) {
		const stack = await this.stackService.newStackMutation({
			projectId: this.projectId,
			branch: {}
		});
		const branchName = stack.heads[0]?.name;
		if (!branchName) {
			// This should never happen, but just in case
			throw new Error('No branch name found for the new stack');
		}
		const outcome = await this.stackService.createCommitMutation({
			projectId: this.projectId,
			stackId: stack.id,
			stackBranchName: branchName,
			parentId: undefined,
			message: STUB_COMMIT_MESSAGE,
			worktreeChanges
		});

		if (outcome.pathsToRejectedChanges.length > 0) {
			showError(
				'Some changes were not committed',
				'The following files were not committed becuase they are locked to another branch:\n' +
					outcome.pathsToRejectedChanges.map(([_reason, path]) => path).join('\n')
			);
		}
		return { stack, outcome, branchName };
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

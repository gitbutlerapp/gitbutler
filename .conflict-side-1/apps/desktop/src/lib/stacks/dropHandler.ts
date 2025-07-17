import { changesToDiffSpec } from '$lib/commits/utils';
import { ChangeDropData } from '$lib/dragging/draggables';
import StackMacros from '$lib/stacks/macros';
import type { DropzoneHandler } from '$lib/dragging/handler';
import type { DiffService } from '$lib/hunks/diffService.svelte';
import type { UncommittedService } from '$lib/selection/uncommittedService.svelte';
import type { StackService } from '$lib/stacks/stackService.svelte';
import type { UiState } from '$lib/state/uiState.svelte';

/** Handler when drop changes on a special outside lanes dropzone. */
export class OutsideLaneDzHandler implements DropzoneHandler {
	private macros: StackMacros;

	constructor(
		private stackService: StackService,
		private projectId: string,
		private readonly uiState: UiState,
		private readonly uncommittedService: UncommittedService,
		private readonly diffService: DiffService
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

				if (!outcome.newCommit) {
					throw new Error('Failed to create a new commit');
				}

				const sourceStackId = data.stackId;
				const sourceCommitId = data.selectionId.commitId;
				if (sourceStackId) {
					const diffSpec = changesToDiffSpec(await data.treeChanges());
					await this.macros.moveChangesToNewCommit(
						stack.id,
						outcome.newCommit,
						sourceStackId,
						sourceCommitId,
						branchName,
						diffSpec
					);
				} else {
					// Should not happen, but just in case
					throw new Error('Change drop data must specify the source stackId');
				}
				break;
			}
			case 'worktree': {
				const stack = await this.stackService.newStackMutation({
					projectId: this.projectId,
					branch: { name: undefined }
				});

				const changes = await data.treeChanges();
				const assignments = changes
					.flatMap((c) => this.uncommittedService.getAssignmentsByPath(data.stackId, c.path))
					.map((h) => ({ ...h, stackId: stack.id }));
				await this.diffService.assignHunk({
					projectId: this.projectId,
					assignments
				});
			}
		}
	}
}

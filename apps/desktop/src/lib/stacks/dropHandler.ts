import { BranchDropData } from '$lib/branches/dropHandler';
import { changesToDiffSpec } from '$lib/commits/utils';
import {
	FileChangeDropData,
	FolderChangeDropData,
	HunkDropDataV3,
	effectiveHunkHeaders,
	type ChangeDropData
} from '$lib/dragging/draggables';
import { unstackPRs, updateStackPrs } from '$lib/forge/shared/prFooter';
import StackMacros from '$lib/stacks/macros';
import { handleMoveBranchResult } from '$lib/stacks/stack';
import { ensureValue } from '$lib/utils/validation';
import { chipToasts } from '@gitbutler/ui';
import type { DropzoneHandler } from '$lib/dragging/handler';
import type { ForgePrService } from '$lib/forge/interface/forgePrService';
import type { DiffService } from '$lib/hunks/diffService.svelte';
import type { UncommittedService } from '$lib/selection/uncommittedService.svelte';
import type { StackService } from '$lib/stacks/stackService.svelte';
import type { UiState } from '$lib/state/uiState.svelte';

/** Handler when drop changes on a special outside lanes dropzone. */
export class OutsideLaneDzHandler implements DropzoneHandler {
	private macros: StackMacros;

	constructor(
		private stackService: StackService,
		private prService: ForgePrService | undefined,
		private projectId: string,
		private readonly uiState: UiState,
		private readonly uncommittedService: UncommittedService,
		private readonly diffService: DiffService,
		private readonly baseBranchName: string | undefined
	) {
		this.macros = new StackMacros(this.projectId, this.stackService, this.uiState);
	}

	private acceptsChangeDropData(data: unknown): data is ChangeDropData {
		if (!(data instanceof FileChangeDropData || data instanceof FolderChangeDropData)) return false;
		if (data.selectionId.type === 'commit' && data.stackId === undefined) return false;
		return true;
	}

	private acceptsHunkDropData(data: unknown): data is HunkDropDataV3 {
		if (!(data instanceof HunkDropDataV3)) return false;
		if (data.selectionId.type === 'commit' && data.stackId === undefined) return false;
		if (data.selectionId.type === 'branch') return false;
		return true;
	}

	private acceptsBranchDropData(data: unknown): data is BranchDropData {
		if (!(data instanceof BranchDropData)) return false;
		if (data.hasConflicts) return false;
		if (data.numberOfBranchesInStack <= 1) return false; // Can't tear off the last branch of a stack
		if (data.numberOfCommits === 0) return false; // TODO: Allow to rip empty branches
		return true;
	}

	accepts(data: unknown) {
		return (
			this.acceptsChangeDropData(data) ||
			this.acceptsBranchDropData(data) ||
			this.acceptsHunkDropData(data)
		);
	}

	async ondropChangeData(data: ChangeDropData) {
		switch (data.selectionId.type) {
			case 'branch': {
				const newBranchName = await this.stackService.fetchNewBranchName(this.projectId);

				if (!newBranchName) {
					throw new Error('Failed to generate a new branch name.');
				}

				if (!data.stackId) {
					throw new Error('Change drop data must specify the source stackId');
				}

				const sourceStackId = data.stackId;
				const sourceBranchName = data.selectionId.branchName;

				await chipToasts.promise(
					(async () => {
						const fileNames = await data
							.treeChanges()
							.then((changes) => changes.map((c) => c.path));

						await this.stackService.splitBranchMutation({
							projectId: this.projectId,
							sourceStackId,
							sourceBranchName,
							fileChangesToSplitOff: fileNames,
							newBranchName: newBranchName
						});
					})(),
					{
						loading: 'Splitting branch into a new branch...',
						success: 'Branch split successfully',
						error: 'Failed to split branch'
					}
				);

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
						ensureValue(stack.id),
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
					.flatMap((c) =>
						this.uncommittedService.getAssignmentsByPath(data.stackId ?? null, c.path)
					)
					.map((h) => ({ ...h, stackId: ensureValue(stack.id) }));
				await this.diffService.assignHunk({
					projectId: this.projectId,
					assignments
				});
			}
		}
	}

	async ondropHunkData(data: HunkDropDataV3) {
		switch (data.selectionId.type) {
			case 'commit': {
				if (!data.stackId || !data.commitId) {
					throw new Error('Hunk drop data must specify the source stackId and commitId');
				}

				const { stack, outcome, branchName } = await this.macros.createNewStackAndCommit();

				if (!outcome.newCommit) {
					throw new Error('Failed to create a new commit');
				}

				const previousPathBytes =
					data.change.status.type === 'Rename'
						? data.change.status.subject.previousPathBytes
						: null;

				await this.macros.moveChangesToNewCommit(
					ensureValue(stack.id),
					outcome.newCommit,
					data.stackId,
					data.commitId,
					branchName,
					[
						{
							previousPathBytes,
							pathBytes: data.change.pathBytes,
							hunkHeaders: effectiveHunkHeaders(data)
						}
					]
				);
				break;
			}
			case 'worktree': {
				const stack = await this.stackService.newStackMutation({
					projectId: this.projectId,
					branch: { name: undefined }
				});

				const assignmentReactive = this.uncommittedService.getAssignmentByHeader(
					data.stackId,
					data.change.path,
					data.hunk
				);
				const assignment = assignmentReactive.current;
				if (!assignment) {
					throw new Error('No hunk assignment found for the dropped worktree hunk');
				}

				await this.diffService.assignHunk({
					projectId: this.projectId,
					assignments: [{ ...assignment, stackId: ensureValue(stack.id) }]
				});
				break;
			}
		}
	}

	async ondropBranchData(data: BranchDropData) {
		await this.stackService
			.tearOffBranch({
				projectId: this.projectId,
				sourceStackId: data.stackId,
				subjectBranchName: data.branchName
			})
			.then(async (result) => {
				handleMoveBranchResult(result);
				return await this.updatePrDescriptions(data);
			});
	}

	private async updatePrDescriptions(data: BranchDropData) {
		if (this.prService === undefined) return;
		if (data.prNumber === undefined) return;
		if (this.baseBranchName === undefined) return;
		const prs = [data.prNumber, ...data.allOtherPrNumbersInStack];

		if (data.allOtherPrNumbersInStack.length === 1) {
			await unstackPRs(this.prService, prs, this.baseBranchName);
			return;
		}

		await unstackPRs(this.prService, [data.prNumber], this.baseBranchName);
		const branchDetails = await this.stackService.fetchBranches(this.projectId, data.stackId);
		await updateStackPrs(this.prService, branchDetails, this.baseBranchName);
	}

	async ondrop(data: unknown): Promise<void> {
		if (this.acceptsChangeDropData(data)) {
			await this.ondropChangeData(data);
			return;
		}

		if (this.acceptsHunkDropData(data)) {
			await this.ondropHunkData(data);
			return;
		}

		if (this.acceptsBranchDropData(data)) {
			await this.ondropBranchData(data);
			return;
		}
	}
}

import { changesToDiffSpec } from "$lib/commits/utils";
import {
	FileChangeDropData,
	FolderChangeDropData,
	HunkDropDataV3,
	type ChangeDropData,
} from "$lib/dragging/draggables";
import { BranchDropData } from "$lib/dragging/dropHandlers/branchDropHandler";
import { CommitDropData } from "$lib/dragging/dropHandlers/commitDropHandler";
import { parseError } from "$lib/error/parser";
import { unstackPRs, updateStackPrs } from "$lib/forge/shared/prFooter";
import { toCommitMovePlacement } from "$lib/stacks/commitMovePlacement";
import StackMacros from "$lib/stacks/macros";
import { toMoveBranchWarning } from "$lib/stacks/stack";
import { withStackBusy } from "$lib/state/uiState.svelte";
import { ensureValue } from "$lib/utils/validation";
import { untrack } from "svelte";
import type { DropResult } from "$lib/dragging/dropResult";
import type { DropzoneHandler } from "$lib/dragging/handler";
import type { ForgePrService } from "$lib/forge/interface/forgePrService";
import type { DiffService } from "$lib/hunks/diffService.svelte";
import type { UncommittedService } from "$lib/selection/uncommittedService.svelte";
import type { StackService } from "$lib/stacks/stackService.svelte";
import type { UiState } from "$lib/state/uiState.svelte";
import type { HunkAssignmentTarget } from "@gitbutler/but-sdk";

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
		private readonly baseBranchName: string | undefined,
	) {
		this.macros = new StackMacros(this.projectId, this.stackService, this.uiState);
	}

	private stackTarget(stackId: string): HunkAssignmentTarget {
		return { type: "stack", subject: { stackId } };
	}

	private acceptsChangeDropData(data: unknown): data is ChangeDropData {
		if (!(data instanceof FileChangeDropData || data instanceof FolderChangeDropData)) return false;
		if (data.selectionId.type === "commit" && data.stackId === undefined) return false;
		if (data.selectionId.type === "branch") return false;
		return true;
	}

	private acceptsHunkDropData(data: unknown): data is HunkDropDataV3 {
		if (!(data instanceof HunkDropDataV3)) return false;
		if (data.selectionId.type === "commit" && data.stackId === undefined) return false;
		if (data.selectionId.type === "branch") return false;
		return true;
	}

	private acceptsBranchDropData(data: unknown): data is BranchDropData {
		if (!(data instanceof BranchDropData)) return false;
		if (data.hasConflicts) return false;
		if (data.numberOfBranchesInStack <= 1) return false; // Can't tear off the last branch of a stack
		if (data.numberOfCommits === 0) return false; // TODO: Allow to rip empty branches
		return true;
	}

	private acceptsCommitDropData(data: unknown): data is CommitDropData {
		if (!(data instanceof CommitDropData)) return false;
		if (data.allCommits.some((c) => c.hasConflicts)) return false;
		return true;
	}

	accepts(data: unknown) {
		return (
			this.acceptsChangeDropData(data) ||
			this.acceptsBranchDropData(data) ||
			this.acceptsHunkDropData(data) ||
			this.acceptsCommitDropData(data)
		);
	}

	async ondropChangeData(data: ChangeDropData) {
		switch (data.selectionId.type) {
			case "commit": {
				const { stack, outcome, branchName } = await this.macros.createNewStackAndCommit();

				if (!outcome.newCommit) {
					throw new Error("Failed to create a new commit");
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
						diffSpec,
					);
				} else {
					// Should not happen, but just in case
					throw new Error("Change drop data must specify the source stackId");
				}
				break;
			}
			case "worktree": {
				const stack = await this.stackService.newStackMutation({
					projectId: this.projectId,
					branch: { name: undefined },
				});

				const changes = await data.treeChanges();
				const assignments = changes
					.flatMap((c) =>
						this.uncommittedService.getAssignmentsByPath(data.stackId ?? null, c.path),
					)
					.map((h) => ({
						hunkHeader: h.hunkHeader,
						pathBytes: h.pathBytes,
						target: this.stackTarget(ensureValue(stack.id)),
					}));
				await this.diffService.assignHunk({
					projectId: this.projectId,
					assignments,
				});
			}
		}
	}

	async ondropHunkData(data: HunkDropDataV3) {
		switch (data.selectionId.type) {
			case "commit": {
				if (!data.stackId || !data.commitId) {
					throw new Error("Hunk drop data must specify the source stackId and commitId");
				}

				const { stack, outcome, branchName } = await this.macros.createNewStackAndCommit();

				if (!outcome.newCommit) {
					throw new Error("Failed to create a new commit");
				}

				const previousPathBytes =
					data.change.status.type === "Rename"
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
							hunkHeaders: [
								{
									oldStart: data.hunk.oldStart,
									oldLines: data.hunk.oldLines,
									newStart: data.hunk.newStart,
									newLines: data.hunk.newLines,
								},
							],
						},
					],
				);
				break;
			}
			case "worktree": {
				const stack = await this.stackService.newStackMutation({
					projectId: this.projectId,
					branch: { name: undefined },
				});

				const assignmentReactive = this.uncommittedService.getAssignmentByHeader(
					data.stackId,
					data.change.path,
					data.hunk,
				);
				const assignment = assignmentReactive.current;
				if (!assignment) {
					throw new Error("No hunk assignment found for the dropped worktree hunk");
				}

				await this.diffService.assignHunk({
					projectId: this.projectId,
					assignments: [
						{
							hunkHeader: assignment.hunkHeader,
							pathBytes: assignment.pathBytes,
							target: this.stackTarget(ensureValue(stack.id)),
						},
					],
				});
				break;
			}
		}
	}

	async ondropCommitData(data: CommitDropData): Promise<DropResult | void> {
		// Clear the selection from the source lane if any dragged commit was selected.
		const sourceSelection = untrack(() => this.uiState.lane(data.stackId).selection.current);
		if (
			sourceSelection?.commitId &&
			data.allCommits.some((c) => c.id === sourceSelection.commitId)
		) {
			this.uiState.lane(data.stackId).selection.set(undefined);
		}

		const stack = await this.stackService.newStackMutation({
			projectId: this.projectId,
			branch: { name: undefined },
		});

		const stackId = ensureValue(stack.id);
		const branchName = ensureValue(stack.heads.at(0)?.name);

		const { relativeTo, side } = toCommitMovePlacement({
			targetBranchName: branchName,
			targetCommitId: "top",
		});

		const commitIds = data.allCommits.map((c) => c.id);
		let result: DropResult | undefined;
		await withStackBusy(
			this.uiState,
			this.projectId,
			{ stackIds: [data.stackId, stackId] },
			async () => {
				try {
					await this.stackService.commitMove({
						projectId: this.projectId,
						subjectCommitIds: commitIds,
						relativeTo,
						side,
						dryRun: false,
					});
				} catch (error) {
					const { description, message } = parseError(error);
					result = {
						type: "warning",
						title: "Cannot move commits",
						message: description ?? message,
					};
				}
			},
		);
		return result;
	}

	async ondropBranchData(data: BranchDropData): Promise<DropResult | void> {
		const beforeAppliedStackCount = (await this.stackService.fetchStacks(this.projectId)).length;
		const result = await this.stackService.tearOffBranch({
			projectId: this.projectId,
			sourceStackId: data.stackId,
			subjectBranchName: data.branchName,
		});
		const afterAppliedStackCount = result.workspace.headInfo.stacks.length;
		const unappliedStackCount = Math.max(0, beforeAppliedStackCount + 1 - afterAppliedStackCount);
		await this.updatePrDescriptions(data);
		return toMoveBranchWarning(unappliedStackCount);
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

	async ondrop(data: unknown): Promise<DropResult | void> {
		if (this.acceptsChangeDropData(data)) {
			await this.ondropChangeData(data);
			return;
		}

		if (this.acceptsHunkDropData(data)) {
			await this.ondropHunkData(data);
			return;
		}

		if (this.acceptsCommitDropData(data)) {
			return await this.ondropCommitData(data);
		}

		if (this.acceptsBranchDropData(data)) {
			return await this.ondropBranchData(data);
		}
	}
}

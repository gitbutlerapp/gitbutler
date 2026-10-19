import { FileChangeDropData, FolderChangeDropData, HunkDropDataV3 } from "$lib/dragging/draggables";
import { UNCOMMITTED_SERVICE } from "$lib/selection/uncommittedService.svelte";
import { normalizeReferenceSubject } from "$lib/stacks/commitMovePlacement";
import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
import { UI_STATE } from "$lib/state/uiState.svelte";
import { inject } from "@gitbutler/core/context";
import type { DropResult } from "$lib/dragging/dropResult";
import type { DropzoneHandler } from "$lib/dragging/handler";
import type { InsertSide, RelativeTo } from "@gitbutler/but-sdk";

export class BranchDropData {
	constructor(
		readonly stackId: string | undefined,
		readonly branchName: string,
		readonly hasConflicts: boolean,
		readonly numberOfBranchesInStack: number,
		readonly numberOfCommits: number,
		readonly prNumber: number | undefined,
		readonly allOtherPrNumbersInStack: number[],
		/** The linked worktree whose lane the branch is in, if not a stack's. */
		readonly worktree?: string,
	) {}

	print(): string {
		return `BranchDropData(${this.stackId}, ${this.branchName}, ${this.hasConflicts})`;
	}
}

/**
 * Where a branch dropped on `targetBranchName`'s zone goes: above the branch, or
 * for the zone under the branch, below its last commit. Below the branch's
 * reference would put the moved branch between the reference and its commits.
 */
export function moveBranchPlacement(
	targetBranchName: string,
	belowCommitId?: string,
): { relativeTo: RelativeTo; side: InsertSide } {
	return belowCommitId === undefined
		? {
				relativeTo: { type: "reference", subject: normalizeReferenceSubject(targetBranchName) },
				side: "above",
			}
		: { relativeTo: { type: "commit", subject: belowCommitId }, side: "below" };
}

export function acceptsSameStackBranchDrop(
	data: BranchDropData,
	targetBranchName: string,
): boolean {
	return data.branchName !== targetBranchName;
}

export class MoveBranchDzHandler implements DropzoneHandler {
	private readonly stackService = inject(STACK_SERVICE);

	constructor(
		private readonly projectId: string,
		private readonly stackId: string | undefined,
		private readonly branchName: string,
		/** The linked worktree whose lane this is, if not a stack's. */
		private readonly worktree?: string,
		/** The branch's last commit: this is the zone under the branch, not above it. */
		private readonly belowCommitId?: string,
	) {}

	print(): string {
		return `MoveBranchDzHandler(${this.projectId}, ${this.stackId}, ${this.branchName})`;
	}

	accepts(data: unknown): boolean {
		if (!(data instanceof BranchDropData) || data.hasConflicts) {
			return false;
		}
		// A worktree keeps at least the branch it has checked out.
		if (data.worktree !== undefined && data.numberOfBranchesInStack <= 1) return false;
		// Dropping a (non-empty) branch onto a different lane merges it into that lane.
		if (data.stackId !== this.stackId || data.worktree !== this.worktree) {
			return data.numberOfCommits > 0;
		}
		return acceptsSameStackBranchDrop(data, this.branchName);
	}
	async ondrop(data: BranchDropData): Promise<DropResult | void> {
		await this.stackService.moveBranch({
			projectId: this.projectId,
			subjectBranch: normalizeReferenceSubject(data.branchName),
			...moveBranchPlacement(this.branchName, this.belowCommitId),
		});
	}
}

export class StartCommitDzHandler implements DropzoneHandler {
	private readonly uiState = inject(UI_STATE);
	private readonly uncommittedService = inject(UNCOMMITTED_SERVICE);

	constructor(
		private readonly projectId: string,
		private readonly stackId: string | undefined,
		private readonly branchName: string,
	) {}

	print(): string {
		return `StartCommitDzHandler(${this.projectId}, ${this.stackId}, ${this.branchName})`;
	}

	accepts(data: unknown): boolean {
		if (data instanceof FileChangeDropData || data instanceof FolderChangeDropData) {
			// Only accept uncomitted files/folders
			if (data.isCommitted) return false;
			// Only the main checkout's, which a stack's branch commits
			if (data.selectionId.worktree !== undefined) return false;
			// Only accept unassinged files/folders or those assigned to the same stack
			if (data.stackId !== undefined && data.stackId !== this.stackId) return false;
			return true;
		}
		if (data instanceof HunkDropDataV3) {
			// Only accept uncommitted hunks
			if (!data.uncommitted) return false;
			if (data.selectionId.type !== "worktree") return false;
			// Only accept unassigned hunks or those assigned to the same stack
			if (data.stackId !== undefined && data.stackId !== this.stackId) return false;
			return true;
		}
		return false;
	}

	private startCommitting() {
		const projectState = this.uiState.project(this.projectId);
		projectState.exclusiveAction.set({
			type: "commit",
			stackId: this.stackId,
			branchName: this.branchName,
		});
	}

	private async checkDropData(
		data: FileChangeDropData | FolderChangeDropData | HunkDropDataV3,
	): Promise<true> {
		if (data instanceof FileChangeDropData || data instanceof FolderChangeDropData) {
			const changes = await data.treeChanges();
			const paths = changes.map((c) => c.path);
			if (paths.length === 0) return true;
			this.uncommittedService.checkFiles(data.stackId ?? null, paths);
			return true;
		}

		// Handle hunk data
		this.uncommittedService.checkHunk(data.stackId ?? null, data.change.path, data.hunk);
		return true;
	}

	async ondrop(data: FileChangeDropData | FolderChangeDropData | HunkDropDataV3): Promise<void> {
		this.startCommitting();
		await this.checkDropData(data);
	}
}

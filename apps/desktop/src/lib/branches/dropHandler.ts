import { FileChangeDropData, FolderChangeDropData, HunkDropDataV3 } from "$lib/dragging/draggables";
import { updateStackPrs } from "$lib/forge/shared/prFooter";
import type { DropzoneHandler } from "$lib/dragging/handler";
import type { ForgePrService } from "$lib/forge/interface/forgePrService";
import type { UncommittedService } from "$lib/selection/uncommittedService.svelte";
import type { StackService } from "$lib/stacks/stackService.svelte";
import type { UiState } from "$lib/state/uiState.svelte";

export class BranchDropData {
	constructor(
		readonly stackId: string,
		readonly branchName: string,
		readonly hasConflicts: boolean,
		readonly numberOfBranchesInStack: number,
		readonly numberOfCommits: number,
		readonly prNumber: number | undefined,
		readonly allOtherPrNumbersInStack: number[],
	) {}

	print(): string {
		return `BranchDropData(${this.stackId}, ${this.branchName}, ${this.hasConflicts})`;
	}
}

export class MoveBranchDzHandler implements DropzoneHandler {
	constructor(
		private readonly stackService: StackService,
		private readonly prService: ForgePrService | undefined,
		private readonly projectId: string,
		private readonly stackId: string,
		private readonly branchName: string,
		private readonly baseBranchName: string | undefined,
	) {}

	print(): string {
		return `MoveBranchDzHandler(${this.projectId}, ${this.stackId}, ${this.branchName})`;
	}

	accepts(data: unknown): boolean {
		return (
			data instanceof BranchDropData &&
			data.stackId !== this.stackId &&
			!data.hasConflicts &&
			data.numberOfCommits > 0 // TODO: If trying to move an empty branch, we should just delete the reference and recreate it.
		);
	}
	async ondrop(data: BranchDropData): Promise<void> {
		const { deletedStacks } = await this.stackService.moveBranch({
			projectId: this.projectId,
			sourceStackId: data.stackId,
			subjectBranchName: data.branchName,
			targetBranchName: this.branchName,
			targetStackId: this.stackId,
		});

		if (!this.prService) return;
		if (!this.baseBranchName) return;

		if (!deletedStacks.includes(data.stackId)) {
			const branchDetails = await this.stackService.fetchBranches(this.projectId, data.stackId);
			await updateStackPrs(this.prService, branchDetails, this.baseBranchName);
		}

		const branchDetails = await this.stackService.fetchBranches(this.projectId, this.stackId);
		await updateStackPrs(this.prService, branchDetails, this.baseBranchName);
	}
}

export class StartCommitDzHandler implements DropzoneHandler {
	constructor(
		private readonly uiState: UiState,
		private readonly uncommittedService: UncommittedService,
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

import { updateStackPrs } from '$lib/forge/shared/prFooter';
import type { DropzoneHandler } from '$lib/dragging/handler';
import type { ForgePrService } from '$lib/forge/interface/forgePrService';
import type { StackService } from '$lib/stacks/stackService.svelte';

export class BranchDropData {
	constructor(
		readonly stackId: string,
		readonly branchName: string,
		readonly hasConflicts: boolean,
		readonly numberOfBranchesInStack: number,
		readonly numberOfCommits: number,
		readonly prNumber: number | undefined,
		readonly allOtherPrNumbersInStack: number[]
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
		private readonly baseBranchName: string | undefined
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
			targetStackId: this.stackId
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

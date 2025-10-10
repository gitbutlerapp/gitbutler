import type { DropzoneHandler } from '$lib/dragging/handler';
import type { StackService } from '$lib/stacks/stackService.svelte';

export class BranchDropData {
	constructor(
		readonly stackId: string,
		readonly branchName: string,
		readonly hasConflicts: boolean,
		readonly numberOfBranchesInStack: number,
		readonly numberOfCommits: number
	) {}

	print(): string {
		return `BranchDropData(${this.stackId}, ${this.branchName}, ${this.hasConflicts})`;
	}
}

export class MoveBranchDzHandler implements DropzoneHandler {
	constructor(
		private readonly stackService: StackService,
		private readonly projectId: string,
		private readonly stackId: string,
		private readonly branchName: string
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
	ondrop(data: BranchDropData): void {
		this.stackService.moveBranch({
			projectId: this.projectId,
			sourceStackId: data.stackId,
			subjectBranchName: data.branchName,
			targetBranchName: this.branchName,
			targetStackId: this.stackId
		});
	}
}

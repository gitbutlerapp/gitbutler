import { CommitDropData } from '$lib/commits/dropHandler';
import type { BranchStack } from '$lib/branches/branch';
import type { PatchSeries } from '$lib/branches/branch';
import type { StackOrder } from '$lib/branches/branch';
import type { BranchController } from '$lib/branches/branchController';
import type { DropzoneHandler } from '$lib/dragging/handler';

export class ReorderCommitDzHandler implements DropzoneHandler {
	constructor(
		private branchId: string,
		private branchController: BranchController,
		private currentSeries: PatchSeries,
		private series: PatchSeries[],
		public commitId: string
	) {}

	accepts(data: unknown) {
		if (!(data instanceof CommitDropData)) return false;
		if (data.stackId !== this.branchId) return false;

		// Do not show dropzones directly above or below the commit in question
		const distance = distanceBetweenDropzones(
			this.series,
			`${data.branchName}|${data.commit.id}`,
			`${this.currentSeries.name}|${this.commitId}`
		);
		if (distance === 0 || distance === 1) return false;

		return true;
	}

	ondrop(data: CommitDropData) {
		const stackOrder = buildNewStackOrder(
			this.series,
			this.currentSeries,
			data.commit.id,
			this.commitId
		);

		if (stackOrder) {
			this.branchController.reorderStackCommit(data.stackId, stackOrder);
		}
	}
}

export class ReorderCommitDzFactory {
	public series: Map<string, PatchSeries>;

	constructor(
		private branchController: BranchController,
		private stack: BranchStack
	) {
		const seriesMap = new Map();
		this.stack.validSeries.forEach((series) => {
			seriesMap.set(series.name, series);
		});
		this.series = seriesMap;
	}

	top(seriesName: string) {
		const currentSeries = this.series.get(seriesName);
		if (!currentSeries) {
			throw new Error('Series not found');
		}

		return new ReorderCommitDzHandler(
			this.stack.id,
			this.branchController,
			currentSeries,
			this.stack.validSeries,
			'top'
		);
	}

	belowCommit(seriesName: string, commitId: string) {
		const currentSeries = this.series.get(seriesName);
		if (!currentSeries) {
			throw new Error('Series not found');
		}

		return new ReorderCommitDzHandler(
			this.stack.id,
			this.branchController,
			currentSeries,
			this.stack.validSeries,
			commitId
		);
	}
}

export class StackingReorderDropzoneManagerFactory {
	constructor(private branchController: BranchController) {}

	build(stack: BranchStack) {
		return new ReorderCommitDzFactory(this.branchController, stack);
	}
}

export function buildNewStackOrder(
	allSeries: PatchSeries[],
	currentSeries: PatchSeries,
	actorCommitId: string,
	targetCommitId: string
): StackOrder | undefined {
	const branches = allSeries
		.filter((s) => !s.archived)
		.map((s) => ({
			name: s.name,
			commitIds: s.patches.map((p) => p.id)
		}));

	const allCommitIds = branches.flatMap((s) => s.commitIds);

	if (
		targetCommitId !== 'top' &&
		(!allCommitIds.includes(actorCommitId) || !allCommitIds.includes(targetCommitId))
	) {
		throw new Error('Commit not found in series');
	}

	const currentSeriesIndex = branches.findIndex((s) => s.name === currentSeries.name);
	if (currentSeriesIndex === -1) return undefined;

	// Remove actorCommitId from its current position
	branches.forEach((s) => {
		s.commitIds = s.commitIds.filter((id) => id !== actorCommitId);
	});

	const updatedCurrentSeries = branches[currentSeriesIndex];
	if (!updatedCurrentSeries) return undefined;

	// Put actorCommtId in its new position
	if (targetCommitId === 'top') {
		updatedCurrentSeries.commitIds.unshift(actorCommitId);
	} else {
		const targetIndex = updatedCurrentSeries.commitIds.indexOf(targetCommitId);
		updatedCurrentSeries.commitIds.splice(targetIndex + 1, 0, actorCommitId);
	}

	branches[currentSeriesIndex] = updatedCurrentSeries;

	return {
		series: branches
	};
}

function distanceBetweenDropzones(
	allSeries: PatchSeries[],
	actorDropzoneId: string,
	targetDropzoneId: string
) {
	const dropzoneIds = allSeries.flatMap((s) => [
		`${s.name}|top`,
		...s.patches.flatMap((p) => `${s.name}|${p.id}`)
	]);

	if (
		!targetDropzoneId.includes('|top') &&
		(!dropzoneIds.includes(actorDropzoneId) || !dropzoneIds.includes(targetDropzoneId))
	) {
		return 0;
	}

	const actorIndex = dropzoneIds.indexOf(actorDropzoneId);
	const targetIndex = dropzoneIds.indexOf(targetDropzoneId);

	return actorIndex - targetIndex;
}

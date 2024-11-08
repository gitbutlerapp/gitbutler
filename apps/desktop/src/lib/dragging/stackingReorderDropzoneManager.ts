import { DraggableCommit } from '$lib/dragging/draggables';
import type { BranchController } from '$lib/vbranches/branchController';
import type { BranchStack, PatchSeries, StackOrder } from '$lib/vbranches/types';

export class StackingReorderDropzone {
	constructor(
		private branchId: string,
		private branchController: BranchController,
		private currentSeries: PatchSeries,
		private series: PatchSeries[],
		public commitId: string
	) {}

	accepts(data: any) {
		if (!(data instanceof DraggableCommit)) return false;
		if (data.branchId !== this.branchId) return false;

		// Do not show dropzones directly above or below the commit in question
		const distance = distanceBetweenDropzones(
			this.series,
			`${data.seriesName}|${data.commit.id}`,
			`${this.currentSeries.name}|${this.commitId}`
		);
		if (distance === 0 || distance === 1) return false;

		return true;
	}

	onDrop(data: any) {
		if (!(data instanceof DraggableCommit)) return;
		if (data.branchId !== this.branchId) return;

		const stackOrder = buildNewStackOrder(
			this.series,
			this.currentSeries,
			data.commit.id,
			this.commitId
		);

		if (stackOrder) {
			this.branchController.reorderStackCommit(data.branchId, stackOrder);
		}
	}
}

export class StackingReorderDropzoneManager {
	public series: Map<string, PatchSeries>;

	constructor(
		private branchController: BranchController,
		private branch: BranchStack
	) {
		const seriesMap = new Map();
		this.branch.series.forEach((series) => {
			seriesMap.set(series.name, series);
		});
		this.series = seriesMap;
	}

	topDropzone(seriesName: string) {
		const currentSeries = this.series.get(seriesName);
		if (!currentSeries) {
			throw new Error('Series not found');
		}

		return new StackingReorderDropzone(
			this.branch.id,
			this.branchController,
			currentSeries,
			this.branch.series,
			'top'
		);
	}

	dropzoneBelowCommit(seriesName: string, commitId: string) {
		const currentSeries = this.series.get(seriesName);
		if (!currentSeries) {
			throw new Error('Series not found');
		}

		return new StackingReorderDropzone(
			this.branch.id,
			this.branchController,
			currentSeries,
			this.branch.series,
			commitId
		);
	}
}

export class StackingReorderDropzoneManagerFactory {
	constructor(private branchController: BranchController) {}

	build(branch: BranchStack) {
		return new StackingReorderDropzoneManager(this.branchController, branch);
	}
}

export function buildNewStackOrder(
	allSeries: PatchSeries[],
	currentSeries: PatchSeries,
	actorCommitId: string,
	targetCommitId: string
): StackOrder | undefined {
	const patchSeries = allSeries.map((s) => ({
		name: s.name,
		commitIds: s.patches.map((p) => p.id)
	}));

	const allCommitIds = patchSeries.flatMap((s) => s.commitIds);

	if (
		targetCommitId !== 'top' &&
		(!allCommitIds.includes(actorCommitId) || !allCommitIds.includes(targetCommitId))
	) {
		throw new Error('Commit not found in series');
	}

	const currentSeriesIndex = patchSeries.findIndex((s) => s.name === currentSeries.name);
	if (currentSeriesIndex === -1) return undefined;

	// Remove actorCommitId from its current position
	patchSeries.forEach((s) => {
		s.commitIds = s.commitIds.filter((id) => id !== actorCommitId);
	});

	const updatedCurrentSeries = patchSeries[currentSeriesIndex];
	if (!updatedCurrentSeries) return undefined;

	// Put actorCommtId in its new position
	if (targetCommitId === 'top') {
		updatedCurrentSeries.commitIds.unshift(actorCommitId);
	} else {
		const targetIndex = updatedCurrentSeries.commitIds.indexOf(targetCommitId);
		updatedCurrentSeries.commitIds.splice(targetIndex + 1, 0, actorCommitId);
	}

	patchSeries[currentSeriesIndex] = updatedCurrentSeries;

	return {
		series: patchSeries
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

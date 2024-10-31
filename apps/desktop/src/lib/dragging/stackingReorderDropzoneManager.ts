import { DraggableCommit } from '$lib/dragging/draggables';
import type { BranchController } from '$lib/vbranches/branchController';
import type { VirtualBranch, PatchSeries, StackOrder } from '$lib/vbranches/types';

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
		if (
			this.commitId !== 'top' &&
			distanceBetweenCommits(this.series, data.commit.id, this.commitId) === 0
		)
			return false;

		return true;
	}

	onDrop(data: any) {
		const allSeriesCommits = this.series.map((s) => ({
			name: s.name,
			commitIds: s.patches.map((p) => p.id)
		}));

		const flatCommits = allSeriesCommits.flatMap((s) => s.commitIds);

		if (!(data instanceof DraggableCommit)) return;
		if (data.branchId !== this.branchId) return;
		if (!flatCommits.find((p) => p === data.commit.id)) return;

		const stackOrder = getTargetStackOrder(
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
		private branch: VirtualBranch
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

	build(branch: VirtualBranch) {
		return new StackingReorderDropzoneManager(this.branchController, branch);
	}
}

function getTargetStackOrder(
	allSeries: PatchSeries[],
	currentSeries: PatchSeries,
	actorCommitId: string,
	targetCommitId: string
): StackOrder | undefined {
	const allSeriesCommits = allSeries.map((s) => ({
		name: s.name,
		commitIds: s.patches.map((p) => p.id)
	}));

	const flatCommits = allSeriesCommits.flatMap((s) => s.commitIds);

	if (
		targetCommitId !== 'top' &&
		(!flatCommits.includes(actorCommitId) || !flatCommits.includes(targetCommitId))
	) {
		throw new Error('Commit not found in series');
	}

	const currentSeriesIndex = allSeriesCommits.findIndex((s) => s.name === currentSeries.name);
	if (currentSeriesIndex === -1) return undefined;

	// Remove actorCommitId from its current position
	allSeriesCommits.forEach((s) => {
		s.commitIds = s.commitIds.filter((id) => id !== actorCommitId);
	});

	const updatedCurrentSeries = allSeriesCommits[currentSeriesIndex];
	if (!updatedCurrentSeries) return undefined;

	// Put actorCommtId in its new position
	if (targetCommitId === 'top') {
		updatedCurrentSeries.commitIds.unshift(actorCommitId);
	} else {
		const targetIndex = updatedCurrentSeries.commitIds.indexOf(targetCommitId);
		updatedCurrentSeries.commitIds.splice(targetIndex + 1, 0, actorCommitId);
	}

	allSeriesCommits[currentSeriesIndex] = updatedCurrentSeries;

	return {
		series: allSeriesCommits
	};
}

function distanceBetweenCommits(
	allSeries: PatchSeries[],
	actorCommitId: string,
	targetCommitId: string
) {
	const allSeriesCommitsFlat = allSeries.flatMap((s) => s.patches.flatMap((p) => p.id));

	if (
		targetCommitId !== 'top' &&
		(!allSeriesCommitsFlat.includes(actorCommitId) ||
			!allSeriesCommitsFlat.includes(targetCommitId))
	) {
		return 0;
	}

	const actorIndex = allSeriesCommitsFlat.indexOf(actorCommitId);
	const targetIndex = allSeriesCommitsFlat.indexOf(targetCommitId);

	return actorIndex - targetIndex;
}

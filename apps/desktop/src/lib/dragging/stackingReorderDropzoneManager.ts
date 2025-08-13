import { CommitDropData } from '$lib/commits/dropHandler';
import { InjectionToken } from '@gitbutler/shared/context';
import type { StackOrder } from '$lib/branches/branch';
import type { DropzoneHandler } from '$lib/dragging/handler';
import type { StackService } from '$lib/stacks/stackService.svelte';

export class ReorderCommitDzHandler implements DropzoneHandler {
	constructor(
		private projectId: string,
		private branchId: string,
		private stackService: StackService,
		private currentSeriesName: string,
		private series: { name: string; commitIds: string[] }[],
		public commitId: string
	) {}

	accepts(data: unknown) {
		if (!(data instanceof CommitDropData)) return false;
		if (data.stackId !== this.branchId) return false;

		// Do not show dropzones directly above or below the commit in question
		const distance = distanceBetweenDropzones(
			this.series,
			`${data.branchName}|${data.commit.id}`,
			`${this.currentSeriesName}|${this.commitId}`
		);
		if (distance === 0 || distance === 1) return false;

		return true;
	}

	async ondrop(data: CommitDropData) {
		const stackOrder = buildNewStackOrder(
			this.series,
			this.currentSeriesName,
			data.commit.id,
			this.commitId
		);

		if (stackOrder) {
			await this.stackService.reorderStack({
				projectId: this.projectId,
				stackId: data.stackId,
				stackOrder
			});
		}
	}
}

export class ReorderCommitDzFactory {
	public series: Map<string, { name: string; commitIds: string[] }>;

	constructor(
		private projectId: string,
		private stackService: StackService,
		private stack: { name: string; commitIds: string[] }[],
		private stackId: string
	) {
		const seriesMap = new Map();
		this.stack.forEach((series) => {
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
			this.projectId,
			this.stackId,
			this.stackService,
			currentSeries.name,
			this.stack,
			'top'
		);
	}

	belowCommit(seriesName: string, commitId: string) {
		const currentSeries = this.series.get(seriesName);
		if (!currentSeries) {
			throw new Error('Series not found');
		}

		return new ReorderCommitDzHandler(
			this.projectId,
			this.stackId,
			this.stackService,
			currentSeries.name,
			this.stack,
			commitId
		);
	}
}

export const STACKING_REORDER_DROPZONE_MANAGER_FACTORY =
	new InjectionToken<StackingReorderDropzoneManagerFactory>(
		'StackingReorderDropzoneManagerFactory'
	);

export class StackingReorderDropzoneManagerFactory {
	constructor(
		private projectId: string,
		private stackService: StackService
	) {}

	build(stackId: string, series: { name: string; commitIds: string[] }[]) {
		return new ReorderCommitDzFactory(this.projectId, this.stackService, series, stackId);
	}
}

function buildNewStackOrder(
	allSeries: { name: string; commitIds: string[] }[],
	currentSeriesName: string,
	actorCommitId: string,
	targetCommitId: string
): StackOrder | undefined {
	const branches = allSeries.map((s) => ({
		name: s.name,
		commitIds: s.commitIds
	}));

	const allCommitIds = branches.flatMap((s) => s.commitIds);

	if (
		targetCommitId !== 'top' &&
		(!allCommitIds.includes(actorCommitId) || !allCommitIds.includes(targetCommitId))
	) {
		throw new Error('Commit not found in series');
	}

	const currentSeriesIndex = branches.findIndex((s) => s.name === currentSeriesName);
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
	allSeries: { name: string; commitIds: string[] }[],
	actorDropzoneId: string,
	targetDropzoneId: string
) {
	const dropzoneIds = allSeries.flatMap((s) => [
		`${s.name}|top`,
		...s.commitIds.flatMap((p) => `${s.name}|${p}`)
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

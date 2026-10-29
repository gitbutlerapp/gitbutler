import { CommitDropData } from "$lib/dragging/dropHandlers/commitDropHandler";
import { toCommitMovePlacement } from "$lib/stacks/commitMovePlacement";
import { withStackBusy, type UiState } from "$lib/state/uiState.svelte";
import { InjectionToken } from "@gitbutler/core/context";
import type { DropzoneHandler } from "$lib/dragging/handler";
import type { StackService } from "$lib/stacks/stackService.svelte";

export class ReorderCommitDzHandler implements DropzoneHandler {
	constructor(
		private projectId: string,
		private branchId: string,
		private stackService: StackService,
		private uiState: UiState,
		private currentSeriesName: string,
		private series: { name: string; commitIds: string[] }[],
		public commitId: string,
	) {}

	accepts(data: unknown) {
		if (!(data instanceof CommitDropData)) return false;
		if (data.isMultiCommit) return false;
		if (data.stackId !== this.branchId) return false;

		// Do not show dropzones directly above or below the commit in question
		const distance = distanceBetweenDropzones(
			this.series,
			`${data.branchName}|${data.commit.id}`,
			`${this.currentSeriesName}|${this.commitId}`,
		);
		if (distance === 0 || distance === 1) return false;

		return true;
	}

	async ondrop(data: CommitDropData) {
		const { side, relativeTo } = toCommitMovePlacement({
			targetBranchName: this.currentSeriesName,
			targetCommitId: this.commitId,
		});
		await withStackBusy(
			this.uiState,
			this.projectId,
			{ commitId: data.commit.id, stackIds: [data.stackId] },
			async () => {
				await this.stackService.commitMove({
					projectId: this.projectId,
					subjectCommitIds: [data.commit.id],
					relativeTo,
					side,
					dryRun: false,
				});
			},
		);
	}
}

export class ReorderCommitDzFactory {
	public series: Map<string, { name: string; commitIds: string[] }>;

	constructor(
		private projectId: string,
		private stackService: StackService,
		private uiState: UiState,
		private stack: { name: string; commitIds: string[] }[],
		private laneId: string,
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
			throw new Error("Series not found");
		}

		return new ReorderCommitDzHandler(
			this.projectId,
			this.laneId,
			this.stackService,
			this.uiState,
			currentSeries.name,
			this.stack,
			"top",
		);
	}

	belowCommit(seriesName: string, commitId: string) {
		const currentSeries = this.series.get(seriesName);
		if (!currentSeries) {
			throw new Error("Series not found");
		}

		return new ReorderCommitDzHandler(
			this.projectId,
			this.laneId,
			this.stackService,
			this.uiState,
			currentSeries.name,
			this.stack,
			commitId,
		);
	}
}

export const REORDER_DROPZONE_FACTORY = new InjectionToken<ReorderDropzoneFactory>(
	"ReorderDropzoneFactory",
);

export class ReorderDropzoneFactory {
	constructor(
		private stackService: StackService,
		private uiState: UiState,
	) {}

	build(projectId: string, laneId: string, series: { name: string; commitIds: string[] }[]) {
		return new ReorderCommitDzFactory(projectId, this.stackService, this.uiState, series, laneId);
	}
}

function distanceBetweenDropzones(
	allSeries: { name: string; commitIds: string[] }[],
	actorDropzoneId: string,
	targetDropzoneId: string,
) {
	const dropzoneIds = allSeries.flatMap((s) => [
		`${s.name}|top`,
		...s.commitIds.flatMap((p) => `${s.name}|${p}`),
	]);

	if (
		!targetDropzoneId.includes("|top") &&
		(!dropzoneIds.includes(actorDropzoneId) || !dropzoneIds.includes(targetDropzoneId))
	) {
		return 0;
	}

	const actorIndex = dropzoneIds.indexOf(actorDropzoneId);
	const targetIndex = dropzoneIds.indexOf(targetDropzoneId);

	return actorIndex - targetIndex;
}

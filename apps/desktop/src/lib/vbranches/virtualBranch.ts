import { DependencyError, PatchSeries, VirtualBranch, VirtualBranches } from './types';
import { invoke, listen } from '$lib/backend/ipc';
import { showError } from '$lib/notifications/toasts';
import { plainToInstance } from 'class-transformer';
import { writable } from 'svelte/store';
import type { BranchListingService } from '$lib/branches/branchListing';
import type { ProjectMetrics } from '$lib/metrics/projectMetrics';
import type { ModeService } from '$lib/modes/service';

export function allPreviousSeriesHavePrNumber(
	seriesName: string,
	validSeries: PatchSeries[]
): boolean {
	const unarchivedSeries = validSeries.filter((series) => !series.archived);
	for (let i = unarchivedSeries.length - 1; i >= 0; i--) {
		const series = unarchivedSeries[i]!;
		if (series.name === seriesName) return true;
		if (series.prNumber === null) return false;
	}

	// Will only happen if the series name is invalid
	// or if the series failed to be fetched.
	return false;
}

export function childBranch(current: PatchSeries, all: PatchSeries[]): PatchSeries | undefined {
	const index = all.indexOf(current);
	if (index === -1 || index === 0) {
		// Either not found or branch is first.
		return undefined;
	}
	return all[index - 1];
}

export function parentBranch(current: PatchSeries, all: PatchSeries[]): PatchSeries | undefined {
	const index = all.indexOf(current);
	if (index === -1 || index === all.length - 1) {
		// Either not found or branch is last.
		return undefined;
	}
	return all[index + 1];
}

export class VirtualBranchService {
	private loading = writable(false);
	readonly error = writable();
	readonly branchesError = writable<any>();

	readonly branches = writable<VirtualBranch[] | undefined>(undefined, () => {
		this.refresh();
		const unsubscribe = this.subscribe(async (branches) => await this.handlePayload(branches));
		return () => {
			unsubscribe();
		};
	});

	constructor(
		private readonly projectId: string,
		private readonly projectMetrics: ProjectMetrics,
		private readonly branchListingService: BranchListingService,
		private readonly modeService: ModeService
	) {}

	async refresh() {
		this.loading.set(true);
		try {
			await this.modeService.awaitNotEditing();
			this.handlePayload(await this.listVirtualBranches());
			this.branchListingService.refresh();
		} catch (err: unknown) {
			console.error(err);
			this.error.set(err);
			showError('Failed to load branches', err);
		} finally {
			this.loading.set(false);
		}
	}

	private async handlePayload(branches: VirtualBranch[]) {
		this.linkRelatedCommits(branches);
		this.branches.set(branches);
		this.branchesError.set(undefined);
		this.updateMetrics(branches);
	}

	/**
	 * For the purpose of showing correct commits in correct colors we often
	 * neeed to know if a commit corresponds to something upstream, such
	 * that we can tell e.g. if a commit has been rebased.
	 */
	private async linkRelatedCommits(branches: VirtualBranch[]) {
		branches.forEach(async (branch) => {
			const upstreamName = branch.upstream?.name;
			if (upstreamName) {
				const upstreamCommits = branch.validSeries.flatMap((series) => series.upstreamPatches);
				const commits = branch.validSeries.flatMap((series) => series.patches);
				commits.forEach((commit) => {
					const upstreamMatch = upstreamCommits.find(
						(upstreamCommit) => commit.remoteCommitId === upstreamCommit.id
					);
					if (upstreamMatch) {
						upstreamMatch.relatedTo = commit;
						commit.relatedTo = upstreamMatch;
					}
				});
			}
		});
	}

	private async listVirtualBranches(): Promise<VirtualBranch[]> {
		const response = await invoke<any>('list_virtual_branches', { projectId: this.projectId });
		const virtualBranches = plainToInstance(VirtualBranches, response);

		if (virtualBranches.dependencyErrors.length > 0) {
			this.handleDependencyErrors(virtualBranches.dependencyErrors);
		}

		return virtualBranches.branches;
	}

	private handleDependencyErrors(errors: DependencyError[]) {
		for (const e of errors) {
			console.error(`Error calculating dependencies:
${e.errorMessage}
Stack: ${e.stackId}
Commit: ${e.commitId}
Path: ${e.path}`);
		}
	}

	private subscribe(callback: (branches: VirtualBranch[]) => void) {
		return listen<any>(`project://${this.projectId}/virtual-branches`, (event) =>
			callback(plainToInstance(VirtualBranches, event.payload).branches)
		);
	}

	private updateMetrics(branches: VirtualBranch[]) {
		try {
			const files = branches.flatMap((branch) => branch.files);
			const hunks = files.flatMap((file) => file.hunks);
			const lockedHunks = hunks.filter((hunk) => hunk.locked);
			this.projectMetrics.setMetric('hunk_count', hunks.length);
			this.projectMetrics.setMetric('locked_hunk_count', lockedHunks.length);
			this.projectMetrics.setMetric('file_count', files.length);
			this.projectMetrics.setMetric('virtual_branch_count', branches.length);
			this.projectMetrics.setMetric(
				'max_stack_count',
				branches.length > 0 ? Math.max(...branches.map((b) => b.series.length)) : 0
			);
		} catch (err: unknown) {
			console.error(err);
		}
	}
}

import {
	BranchStack,
	DetailedCommit,
	Commit,
	ListBranchStacksResponse,
	commitCompare
} from './types';
import { invoke, listen } from '$lib/backend/ipc';
import { RemoteBranchService } from '$lib/stores/remoteBranches';
import { plainToInstance } from 'class-transformer';
import { writable } from 'svelte/store';
import type { BranchListingService } from '$lib/branches/branchListing';
import type { ProjectMetrics } from '$lib/metrics/projectMetrics';

export class VirtualBranchService {
	private loading = writable(false);
	readonly error = writable();
	readonly branchesError = writable<any>();

	readonly branches = writable<BranchStack[] | undefined>(undefined, () => {
		this.refresh();
		const unsubscribe = this.subscribe(async (branches) => await this.handlePayload(branches));
		return () => {
			unsubscribe();
		};
	});

	constructor(
		private projectId: string,
		private projectMetrics: ProjectMetrics,
		private remoteBranchService: RemoteBranchService,
		private branchListingService: BranchListingService
	) {}

	async refresh() {
		this.loading.set(true);
		try {
			this.handlePayload(await this.listVirtualBranches());
		} catch (err: any) {
			console.error(err);
			this.error.set(err);
		} finally {
			this.loading.set(false);
		}
	}

	private async handlePayload(branches: BranchStack[]) {
		await Promise.all(
			branches.map(async (b) => {
				const upstreamName = b.upstream?.name;
				if (upstreamName) {
					try {
						const data = await this.remoteBranchService.getRemoteBranchData(upstreamName);
						const upstreamCommits = data.commits;
						const stackedCommits = b.branches.flatMap((series) => series.patches);

						upstreamCommits.forEach((uc) => {
							const match = b.commits.find((c) => commitCompare(uc, c));
							const stackedMatch = stackedCommits.find((c) => commitCompare(uc, c));
							if (match) {
								match.relatedTo = uc;
								uc.relatedTo = match;
							}
							if (stackedMatch) {
								// This asymmetric difference is not ideal, but gets the job done while
								// we are experimenting with stacking.
								stackedMatch.relatedTo = uc;
							}
						});
						linkAsParentChildren(upstreamCommits);
						linkAsParentChildren(stackedCommits);
						b.upstreamData = data;
					} catch (e: any) {
						console.log(e);
					}
				}
				b.files.sort((a) => (a.conflicted ? -1 : 0));
				// This is always true now
				b.isMergeable = Promise.resolve(true);
				const commits = b.commits;
				linkAsParentChildren(commits);
				return b;
			})
		);

		this.branches.set(branches);

		this.branchesError.set(undefined);
		this.logMetrics(branches);

		this.branchListingService.refresh();
	}

	private async listVirtualBranches(): Promise<BranchStack[]> {
		return plainToInstance(
			ListBranchStacksResponse,
			await invoke<any>('list_virtual_branches', { projectId: this.projectId })
		).branches;
	}

	private subscribe(callback: (branches: BranchStack[]) => void) {
		return listen<any>(`project://${this.projectId}/virtual-branches`, (event) =>
			callback(plainToInstance(ListBranchStacksResponse, event.payload).branches)
		);
	}

	private logMetrics(branches: BranchStack[]) {
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
				Math.max(...branches.map((b) => b.branches.length))
			);
		} catch (err: unknown) {
			console.error(err);
		}
	}
}

function linkAsParentChildren(commits: DetailedCommit[] | Commit[]) {
	for (let j = 0; j < commits.length; j++) {
		const commit = commits[j];
		if (commit && j === 0) {
			commit.next = undefined;
		} else if (commit) {
			const child = commits[j - 1];
			if (child instanceof DetailedCommit) commit.next = child;
			if (child instanceof Commit) commit.next = child;
		}
		if (commit && j !== commits.length - 1) {
			commit.prev = commits[j + 1];
		}
	}
}

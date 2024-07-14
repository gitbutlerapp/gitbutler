import { Branch, Commit, RemoteCommit, VirtualBranches, commitCompare } from './types';
import { invoke, listen } from '$lib/backend/ipc';
import { RemoteBranchService } from '$lib/stores/remoteBranches';
import { plainToInstance } from 'class-transformer';
import { writable } from 'svelte/store';
import type { ProjectMetrics } from '$lib/metrics/projectMetrics';

export class VirtualBranchService {
	private _branches: Branch[] = [];
	private loading = writable(false);
	readonly error = writable();
	readonly branchesError = writable<any>();

	readonly branches = writable<Branch[] | undefined>(undefined, () => {
		this.refresh();
		const unsubscribe = this.subscribe(async (branches) => await this.handlePayload(branches));
		return () => {
			unsubscribe();
		};
	});

	constructor(
		private projectId: string,
		private projectMetrics: ProjectMetrics,
		private remoteBranchService: RemoteBranchService
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

	private async handlePayload(branches: Branch[]) {
		await Promise.all(
			branches.map(async (b) => {
				const upstreamName = b.upstream?.name;
				if (upstreamName) {
					try {
						const data = await this.remoteBranchService.getRemoteBranchData(upstreamName);
						const commits = data.commits;
						commits.forEach((uc) => {
							const match = b.commits.find((c) => commitCompare(uc, c));
							if (match) {
								match.relatedTo = uc;
								uc.relatedTo = match;
							}
						});
						linkAsParentChildren(commits);
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
		this.projectMetrics.setMetric('virtual_branch_count', branches.length);
		this._branches = branches;
		this.branches.set(branches);
		this.branchesError.set(undefined);
	}

	async listVirtualBranches(): Promise<Branch[]> {
		return plainToInstance(
			VirtualBranches,
			await invoke<any>('list_virtual_branches', { projectId: this.projectId })
		).branches;
	}

	private subscribe(callback: (branches: Branch[]) => void) {
		return listen<any>(`project://${this.projectId}/virtual-branches`, (event) =>
			callback(plainToInstance(VirtualBranches, event.payload).branches)
		);
	}

	async getById(branchId: string) {
		return this._branches?.find((b) => b.id === branchId && b.upstream);
	}

	async getByUpstreamSha(upstreamSha: string) {
		return this._branches.map((b) => b.upstream?.sha === upstreamSha);
	}
}

function linkAsParentChildren(commits: Commit[] | RemoteCommit[]) {
	for (let j = 0; j < commits.length; j++) {
		const commit = commits[j];
		if (j === 0) {
			commit.next = undefined;
		} else {
			const child = commits[j - 1];
			if (child instanceof Commit) commit.next = child;
			if (child instanceof RemoteCommit) commit.next = child;
		}
		if (j !== commits.length - 1) {
			commit.prev = commits[j + 1];
		}
	}
}

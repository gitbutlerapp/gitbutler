import { invoke } from '$lib/backend/ipc';
import { Branch, BranchData } from '$lib/vbranches/types';
import { plainToInstance } from 'class-transformer';
import { writable } from 'svelte/store';
import type { BranchListingService } from '$lib/branches/branchListing';
import type { ProjectMetrics } from '$lib/metrics/projectMetrics';

export class RemoteBranchService {
	readonly branches = writable<Branch[]>([], () => {
		this.refresh();
	});
	error = writable();

	constructor(
		private projectId: string,
		private branchListingService: BranchListingService,
		private projectMetrics?: ProjectMetrics
	) {}

	async refresh() {
		try {
			const remoteBranches = plainToInstance(
				Branch,
				await invoke<any[]>('list_remote_branches', { projectId: this.projectId })
			);
			this.projectMetrics?.setMetric('normal_branch_count', remoteBranches.length);
			this.branches.set(remoteBranches);
		} catch (err: any) {
			this.error.set(err);
		} finally {
			this.branchListingService.refresh();
		}
	}

	async getRemoteBranchData(refname: string): Promise<BranchData> {
		return plainToInstance(
			BranchData,
			await invoke<any>('get_remote_branch_data', { projectId: this.projectId, refname })
		);
	}
}

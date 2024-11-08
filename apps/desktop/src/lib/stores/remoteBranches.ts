import { Code, invoke } from '$lib/backend/ipc';
import { PartialGitBranch, GitBranch } from '$lib/vbranches/types';
import { plainToInstance } from 'class-transformer';
import { writable } from 'svelte/store';
import type { BranchListingService } from '$lib/branches/branchListing';
import type { ProjectMetrics } from '$lib/metrics/projectMetrics';

export class RemoteBranchService {
	readonly branches = writable<PartialGitBranch[]>([], () => {
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
				PartialGitBranch,
				await invoke<any[]>('list_git_branches', { projectId: this.projectId })
			);
			this.projectMetrics?.setMetric('normal_branch_count', remoteBranches.length);
			this.branches.set(remoteBranches);
		} catch (err: any) {
			if (err.code === Code.DefaultTargetNotFound) {
				// Swallow this error since user should be taken to project setup page
				return;
			}
			this.error.set(err);
		} finally {
			this.branchListingService.refresh();
		}
	}

	async getRemoteBranchData(refname: string): Promise<GitBranch> {
		return plainToInstance(
			GitBranch,
			await invoke<any>('get_remote_branch_data', { projectId: this.projectId, refname })
		);
	}
}

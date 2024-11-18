import { invoke } from '$lib/backend/ipc';
import { Branch, BranchData } from '$lib/vbranches/types';
import { plainToInstance } from 'class-transformer';

export class GitBranchService {
	constructor(private projectId: string) {}

	async findBranches(name: string) {
		return plainToInstance(
			Branch,
			await invoke<any[]>('find_git_branches', { projectId: this.projectId, branchName: name })
		);
	}

	async getRemoteBranchData(refname: string): Promise<BranchData> {
		return plainToInstance(
			BranchData,
			await invoke<any>('get_remote_branch_data', { projectId: this.projectId, refname })
		);
	}
}

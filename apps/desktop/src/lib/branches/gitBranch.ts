import { invoke } from '$lib/backend/ipc';
import { BranchData } from '$lib/vbranches/types';
import { plainToInstance } from 'class-transformer';

export class GitBranchService {
	constructor(private projectId: string) {}

	async findBranches(name: string) {
		return plainToInstance(
			BranchData,
			await invoke<any[]>('find_git_branches', { projectId: this.projectId, branchName: name })
		);
	}
}

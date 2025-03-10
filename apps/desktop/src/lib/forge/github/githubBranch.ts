import type { ForgeBranch } from '../interface/forgeBranch';

export class GitHubBranch implements ForgeBranch {
	readonly url: string;
	constructor(name: string, baseBranch: string, baseUrl: string, fork?: string) {
		if (fork) {
			name = `${fork}:${name}`;
		}
		this.url = `${baseUrl}/compare/${baseBranch}...${name}`;
	}

	/**
	 * @desc Delete branch from remote only
	 */
	async delete(ref: string) {
		this.octokit.delete();
	}
}

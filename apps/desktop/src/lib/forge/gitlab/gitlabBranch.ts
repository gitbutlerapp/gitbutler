import type { GitHostBranch } from '../interface/forgeBranch';

export class GitLabBranch implements GitHostBranch {
	readonly url: string;
	constructor(name: string, baseBranch: string, baseUrl: string, fork?: string) {
		if (fork) {
			name = `${fork}:${name}`;
		}
		this.url = `${baseUrl}/-/compare/${baseBranch}...${name}`;
	}
}

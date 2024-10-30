import type { GitHostBranch } from '../interface/forgeBranch';

export class AzureBranch implements GitHostBranch {
	readonly url: string;
	constructor(name: string, baseBranch: string, baseUrl: string, fork?: string) {
		if (fork) {
			name = `${fork}:${name}`;
		}
		this.url = `${baseUrl}/branchCompare?baseVersion=GB${baseBranch}&targetVersion=GB${name}`;
	}
}

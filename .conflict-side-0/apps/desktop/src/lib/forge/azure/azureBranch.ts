import type { ForgeBranch } from '$lib/forge/interface/forgeBranch';

export class AzureBranch implements ForgeBranch {
	readonly url: string;
	constructor(name: string, baseBranch: string, baseUrl: string, fork?: string) {
		if (fork) {
			name = `${fork}:${name}`;
		}
		this.url = `${baseUrl}/branchCompare?baseVersion=GB${baseBranch}&targetVersion=GB${name}`;
	}
}

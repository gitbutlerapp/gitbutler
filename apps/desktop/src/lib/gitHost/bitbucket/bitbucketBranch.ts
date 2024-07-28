import type { GitHostBranch } from '../interface/gitHostBranch';

export class BitBucketBranch implements GitHostBranch {
	readonly url: string;
	constructor(name: string, baseBranch: string, baseUrl: string, fork?: string) {
		if (fork) {
			name = `${fork}:${name}`;
		}
		this.url = `${baseUrl}/branch/${name}?dest=${baseBranch}`;
	}
}

import type { PullRequest } from '$lib/github/types';
import type { RemoteBranch } from '$lib/vbranches/types';

export class RemoteContribution {
	constructor(
		public remoteBranch: RemoteBranch | undefined,
		public pullRequest: PullRequest | undefined
	) {}
}

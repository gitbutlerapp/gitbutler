import type { DetailedCommit } from '$lib/vbranches/types';

/**
 * Find the last commit that diverged from the remote branch.
 */
export function findLastDivergentCommit(commits: DetailedCommit[]): DetailedCommit | undefined {
	for (let i = commits.length - 1; i >= 0; i--) {
		const commit = commits[i];
		if (commit!.id !== commit!.remoteCommitId) {
			return commit;
		}
	}
	return undefined;
}

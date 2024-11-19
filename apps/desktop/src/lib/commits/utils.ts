import type { DetailedCommit } from '$lib/vbranches/types';

type DivergenceResult =
	| { type: 'localDiverged'; commit: DetailedCommit }
	| { type: 'notDiverged' | 'onlyRemoteDiverged' };

/**
 * Find the last commit that diverged from the remote branch.
 */
export function findLastDivergentCommit(
	remoteCommits: DetailedCommit[],
	commits: DetailedCommit[]
): DivergenceResult {
	const noLocalDiverged = commits.every((commit) => !commit.remoteCommitId);

	// If there are no diverged or remote commits, then there is no last
	// diverged commit.
	if (noLocalDiverged && remoteCommits.length === 0) {
		return { type: 'notDiverged' };
	}

	if (noLocalDiverged) {
		return { type: 'onlyRemoteDiverged' };
	}

	for (let i = commits.length - 1; i >= 0; i--) {
		const commit = commits[i]!;
		if (commit.id !== commit.remoteCommitId) {
			return { type: 'localDiverged', commit };
		}
	}

	return { type: 'notDiverged' };
}

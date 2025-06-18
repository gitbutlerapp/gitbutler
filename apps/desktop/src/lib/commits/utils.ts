import { isDefined } from '@gitbutler/ui/utils/typeguards';
import type { DetailedCommit } from '$lib/commits/commit';
import type { TreeChange } from '$lib/hunks/change';
import type { DiffSpec, HunkAssignment } from '$lib/hunks/hunk';

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
/** Helper function that turns tree changes into a diff spec */
export function changesToDiffSpec(
	changes: TreeChange[],
	assignments?: Record<string, HunkAssignment[]>
): DiffSpec[] {
	return changes.map((change) => {
		const previousPathBytes =
			change.status.type === 'Rename' ? change.status.subject.previousPathBytes : null;
		const assignment = assignments?.[change.path];
		const hunkHeaders = assignment?.map((a) => a.hunkHeader).filter(isDefined) ?? [];

		return {
			previousPathBytes,
			pathBytes: change.pathBytes,
			hunkHeaders
		};
	});
}

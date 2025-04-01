import type { UpstreamCommit, Commit, CommitState } from '$lib/branches/v3';
import type { PushStatus } from '$lib/stacks/stack';

const colorMap = {
	LocalOnly: 'var(--clr-commit-local)',
	LocalAndRemote: 'var(--clr-commit-remote)',
	Integrated: 'var(--clr-commit-integrated)',
	Error: 'var(--clr-theme-err-element)'
};

export function getColorFromCommitState(commitId: string, commitState: CommitState): string {
	if (commitState.type === 'LocalAndRemote' && commitState.subject !== commitId) {
		// Diverged
		return colorMap.LocalOnly;
	}

	return colorMap[commitState.type];
}

export function isUpstreamCommit(commit: Commit | UpstreamCommit): commit is UpstreamCommit {
	return !('state' in commit);
}

export function isLocalAndRemoteCommit(commit: Commit | UpstreamCommit): commit is Commit {
	return 'state' in commit;
}

export function getBranchStatusLabelAndColor(pushStatus: PushStatus): {
	label: string;
	color: string;
} {
	switch (pushStatus) {
		case 'completelyUnpushed':
			return { label: 'Unpushed', color: colorMap.LocalOnly };
		case 'nothingToPush':
			return { label: 'Nothing to push', color: colorMap.LocalAndRemote };
		case 'unpushedCommits':
		case 'unpushedCommitsRequiringForce':
			return { label: 'Unpushed', color: colorMap.LocalAndRemote };
		case 'integrated':
			return { label: 'Integrated', color: colorMap.Integrated };
		default:
			return { label: 'Unknown', color: colorMap.Error };
	}
}

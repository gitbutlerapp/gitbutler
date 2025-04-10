import type { UpstreamCommit, Commit, CommitState } from '$lib/branches/v3';
import type { PushStatus } from '$lib/stacks/stack';
import type iconsJson from '@gitbutler/ui/data/icons.json';

const colorMap = {
	LocalOnly: 'var(--clr-commit-local)',
	LocalAndRemote: 'var(--clr-commit-remote)',
	Integrated: 'var(--clr-commit-integrated)',
	Error: 'var(--clr-theme-err-element)'
};

export function getIconFromCommitState(
	commitId?: string,
	commitState?: CommitState
): keyof typeof iconsJson {
	if (!commitId || !commitState) {
		return 'branch-local';
	}
	switch (commitState.type) {
		case 'LocalOnly':
			return 'branch-local';
		case 'LocalAndRemote':
			return commitState.subject !== commitId ? 'branch-local' : 'branch-remote';
		case 'Integrated':
			return 'tick-small';
		default:
			return 'branch-local';
	}
}

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

export function getCommitType(commit: Commit | UpstreamCommit) {
	const isLocalAndRemote = isLocalAndRemoteCommit(commit) && commit.state.type === 'LocalAndRemote';
	const isDiverged = isLocalAndRemote && commit.state.subject !== commit.id;
	const isUpstream = !isLocalAndRemoteCommit(commit);
	const isLocalOnly = !isLocalAndRemote && !isUpstream;

	// return string
	if (isLocalOnly) {
		return 'local';
	} else if (isLocalAndRemote) {
		return isDiverged ? 'diverged' : 'local-and-remote';
	} else if (isUpstream) {
		return 'upstream';
	}
	return 'local';
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
			return { label: 'Unpushed commits', color: colorMap.LocalAndRemote };
		case 'integrated':
			return { label: 'Integrated', color: colorMap.Integrated };
		default:
			return { label: 'Unknown', color: colorMap.Error };
	}
}

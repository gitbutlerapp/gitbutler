import type { UpstreamCommit, Commit, CommitState } from '$lib/branches/v3';
import type { CommitStatusType } from '$lib/commits/commit';
import type { PushStatus } from '$lib/stacks/stack';
import type iconsJson from '@gitbutler/ui/data/icons.json';

const colorMap: Record<CommitStatusType, string> & { Error: string } = {
	LocalOnly: 'var(--clr-commit-local)',
	LocalAndRemote: 'var(--clr-commit-remote)',
	Integrated: 'var(--clr-commit-integrated)',
	Remote: 'var(--clr-commit-upstream)', // TODO: rename Remote -> Upstream.
	Base: 'var(--clr-commit-upstream)', // TODO: Introduce separate color for base.
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

export function getColorFromCommitState(commitType: CommitStatusType, diverged: boolean): string {
	if (diverged) {
		return colorMap.LocalOnly;
	}

	return colorMap[commitType];
}

export function isUpstreamCommit(commit: Commit | UpstreamCommit): commit is UpstreamCommit {
	return !('state' in commit);
}

export function isLocalAndRemoteCommit(commit: Commit | UpstreamCommit): commit is Commit {
	return 'state' in commit;
}

export function hasConflicts(commit: Commit): boolean {
	return commit.state.type === 'LocalAndRemote' && commit.id !== commit.state.subject;
}

export function isEditableCommit(
	status: CommitStatusType
): status is 'LocalOnly' | 'LocalAndRemote' {
	return status === 'LocalOnly' || status === 'LocalAndRemote';
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

import type { CommitStateType, UpstreamCommit, Commit } from '$lib/branches/v3';

const colorMap = {
	LocalOnly: 'var(--clr-commit-local)',
	LocalAndRemote: 'var(--clr-commit-remote)',
	Integrated: 'var(--clr-commit-integrated)',
	Error: 'var(--clr-theme-err-element)'
};

export function getColorFromBranchType(type: CommitStateType | 'Error'): string {
	return colorMap[type];
}

export function isUpstreamCommit(commit: Commit | UpstreamCommit): commit is UpstreamCommit {
	return !('state' in commit);
}

export function isLocalAndRemoteCommit(commit: Commit | UpstreamCommit): commit is Commit {
	return 'state' in commit;
}

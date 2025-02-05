import type { CommitStateType, State, Commits, UpstreamCommit, Commit } from '$lib/branches/v3';

const colorMap = {
	LocalOnly: 'var(--clr-commit-local)',
	LocalAndRemote: 'var(--clr-commit-remote)',
	Integrated: 'var(--clr-commit-integrated)',
	Error: 'var(--clr-theme-err-element)'
};

export function getColorFromBranchType(type: CommitStateType | 'Error'): string {
	return colorMap[type];
}

export function isArchivedBranch(state: State): state is { type: 'Archived' } {
	return state.type === 'Archived';
}

export function isStackedBranch(state: State): state is { type: 'Stacked'; subject: Commits } {
	return state.type === 'Stacked';
}

export function isUpstreamCommit(commit: Commit | UpstreamCommit): commit is UpstreamCommit {
	return !('state' in commit);
}

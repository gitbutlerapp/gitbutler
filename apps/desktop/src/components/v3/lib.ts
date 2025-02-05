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

/**
 * Type guard to check if a State is of type 'Archived'
 * @param state - The State to check
 * @returns True if the state is 'Archived', false otherwise
 */
export function isArchivedBranch(state: State): state is { type: 'Archived' } {
	return state.type === 'Archived';
}

/**
 * Type guard to check if a State is of type 'Stacked'
 * @param state - The State to check
 * @returns True if the state is 'Stacked', false otherwise
 */
export function isStackedBranch(state: State): state is { type: 'Stacked'; subject: Commits } {
	return state.type === 'Stacked';
}

/**
 * Type guard to check if a commit is an 'UpstreamCommit'
 * @param commit - The commit to check
 * @returns True if the commit is an 'UpstreamCommit', false otherwise
 */
export function isUpstreamCommit(commit: Commit | UpstreamCommit): commit is UpstreamCommit {
	return !('state' in commit);
}

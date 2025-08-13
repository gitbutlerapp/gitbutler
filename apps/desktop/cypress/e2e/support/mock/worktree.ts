import type { WorktreeChanges } from '$lib/hunks/change';

export const MOCK_WORKTREE_CHANGES: WorktreeChanges = {
	changes: [],
	ignoredChanges: [],
	assignments: [],
	assignmentsError: null,
	dependencies: {
		diffs: [],
		errors: []
	},
	dependenciesError: null
};

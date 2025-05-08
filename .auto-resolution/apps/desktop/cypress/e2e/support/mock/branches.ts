import type { Author, BranchListing } from '$lib/branches/branchListing';

export const MOCK_BRANCH_AUTHOR_A: Author = {
	name: 'Branchy McBranchface',
	email: 'branchy@example.com'
};

export const MOCK_BRANCH_LISTING_A: BranchListing = {
	name: 'local-branch-a',
	remotes: [],
	stack: undefined,
	updatedAt: Date.now().toString(),
	lastCommiter: MOCK_BRANCH_AUTHOR_A,
	hasLocal: true
};

export const MOCK_BRANCH_LISTING_B: BranchListing = {
	name: 'local-branch-b',
	remotes: [],
	stack: undefined,
	updatedAt: Date.now().toString(),
	lastCommiter: MOCK_BRANCH_AUTHOR_A,
	hasLocal: true
};

export const MOCK_BRANCH_LISTINGS: BranchListing[] = [MOCK_BRANCH_LISTING_A, MOCK_BRANCH_LISTING_B];

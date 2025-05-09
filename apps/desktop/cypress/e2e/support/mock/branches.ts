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

export function createMockBranchListing(override: Partial<BranchListing>): BranchListing {
	return {
		...MOCK_BRANCH_LISTING_A,
		...override
	};
}

export const MOCK_BRANCH_LISTINGS: BranchListing[] = [MOCK_BRANCH_LISTING_A, MOCK_BRANCH_LISTING_B];

export type GetBranchDetailsParams = {
	projectId: string;
	branchName: string;
	remote?: string;
};

export function isGetBranchDetailsParams(params: unknown): params is GetBranchDetailsParams {
	return (
		typeof params === 'object' &&
		params !== null &&
		'projectId' in params &&
		typeof (params as GetBranchDetailsParams).projectId === 'string' &&
		'branchName' in params &&
		typeof (params as GetBranchDetailsParams).branchName === 'string' &&
		(typeof (params as GetBranchDetailsParams).remote === 'string' ||
			(params as GetBranchDetailsParams).remote === undefined)
	);
}

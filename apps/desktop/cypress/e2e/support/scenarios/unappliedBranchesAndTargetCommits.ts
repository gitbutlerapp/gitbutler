import MockBackend from '../mock/backend';
import { createMockBranchListing } from '../mock/branches';
import { createMockCommit } from '../mock/stacks';

const UNAPPLIED_BRANCH_NAMES = [
	'unapplied-branch-1',
	'unapplied-branch-2',
	'unapplied-branch-3',
	'unapplied-branch-4',
	'unapplied-branch-5',
	'unapplied-branch-6'
];

const TARGET_COMMIT_MESSAGES = [
	'Target commit message 1',
	'Target commit message 2',
	'Target commit message 3',
	'Target commit message 4',
	'Target commit message 5',
	'Target commit message 6'
];

/**
 * In this scenario, there are some unapplied branches and target commits.
 *
 * It's well suited for testing the branches page.
 */
export default class UnappliedBranchesAndTargetCommits extends MockBackend {
	constructor() {
		super();

		this.branchListings = UNAPPLIED_BRANCH_NAMES.map((branchName) =>
			createMockBranchListing({ name: branchName })
		);

		this.baseBranchCommits = TARGET_COMMIT_MESSAGES.map((message, index) =>
			createMockCommit({
				id: `target-commit-${index}`,
				message,
				createdAt: Date.now() - index * 1000
			})
		);
	}
}

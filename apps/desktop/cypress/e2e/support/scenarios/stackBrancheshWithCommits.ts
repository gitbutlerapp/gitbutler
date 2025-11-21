import MockBackend from '../mock/backend';
import {
	createMockBranchDetails,
	createMockCommit,
	createMockStack,
	createMockStackDetails
} from '../mock/stacks';

const MOCK_STACK_A_ID = 'stack-a-id';
const MOCK_SECOND_BRANCH_NAME = 'second-branch';

const MOCK_COMMIT_TITLE_A = 'Initial commit';
const MOCK_COMMIT_MESSAGE_A = 'This is a test commit';

const MOCK_COMMIT_IN_BRANCH_A = createMockCommit({
	id: '444444',
	message: `${MOCK_COMMIT_TITLE_A}\n\n${MOCK_COMMIT_MESSAGE_A}`
});

const MOCK_COMMIT_IN_SECOND_BRANCH = createMockCommit({
	id: '555555',
	message: 'This is a commit in the second branch'
});

const MOCK_STACK_DETAILS_A = createMockStackDetails({
	derivedName: MOCK_STACK_A_ID,
	branchDetails: [
		createMockBranchDetails({ name: MOCK_STACK_A_ID, commits: [MOCK_COMMIT_IN_BRANCH_A] }),
		createMockBranchDetails({
			name: MOCK_SECOND_BRANCH_NAME,
			commits: [MOCK_COMMIT_IN_SECOND_BRANCH]
		})
	]
});

const MOCK_STACK_A = createMockStack({
	id: MOCK_STACK_A_ID,
	heads: [
		{ name: MOCK_STACK_A_ID, tip: MOCK_COMMIT_IN_BRANCH_A.id, isCheckedOut: true },
		{ name: MOCK_SECOND_BRANCH_NAME, tip: MOCK_COMMIT_IN_SECOND_BRANCH.id, isCheckedOut: true }
	],
	tip: MOCK_COMMIT_IN_BRANCH_A.id
});

export default class StackBranchesWithCommits extends MockBackend {
	topBranchName = MOCK_STACK_A_ID;
	bottomBranchName = MOCK_SECOND_BRANCH_NAME;
	constructor() {
		super();
		this.stacks = [MOCK_STACK_A];
		this.stackId = MOCK_STACK_A_ID;
		this.stackDetails.set(MOCK_STACK_A_ID, MOCK_STACK_DETAILS_A);

		this.branchChanges.set(
			MOCK_STACK_A_ID,
			new Map([
				[MOCK_STACK_A_ID, []],
				[MOCK_SECOND_BRANCH_NAME, []]
			])
		);
	}

	getCommitTitle(branchName: string): string {
		switch (branchName) {
			case MOCK_STACK_A_ID:
				return MOCK_COMMIT_TITLE_A;
			case MOCK_SECOND_BRANCH_NAME:
				return MOCK_COMMIT_IN_SECOND_BRANCH.message;
			default:
				throw new Error(`Unknown branch name: ${branchName}`);
		}
	}

	getCommitMessage(branchName: string): string {
		switch (branchName) {
			case MOCK_STACK_A_ID:
				return MOCK_COMMIT_MESSAGE_A;
			case MOCK_SECOND_BRANCH_NAME:
				return '';
			default:
				throw new Error(`Unknown branch name: ${branchName}`);
		}
	}
}

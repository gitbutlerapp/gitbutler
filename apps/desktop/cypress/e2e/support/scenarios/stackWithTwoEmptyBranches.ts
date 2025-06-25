import MockBackend from '../mock/backend';
import { createMockBranchDetails, createMockStackDetails } from '../mock/stacks';
import type { Stack } from '$lib/stacks/stack';

const MOCK_STACK_A_ID = 'stack-a-id';
const OTHER_HEADER_NAME = 'other-header-name';

const MOCK_STACK_A: Stack = {
	order: 0,
	id: MOCK_STACK_A_ID,
	heads: [
		{ name: MOCK_STACK_A_ID, tip: '1234123' },
		{ name: OTHER_HEADER_NAME, tip: '1234134' }
	],
	tip: '1234123'
};

const MOCK_STACK_A_DETAILS = createMockStackDetails({
	derivedName: MOCK_STACK_A_ID,
	branchDetails: [
		createMockBranchDetails({ name: MOCK_STACK_A_ID, commits: [] }),
		createMockBranchDetails({ name: OTHER_HEADER_NAME, commits: [] })
	]
});

export default class StackWithTwoEmptyBranches extends MockBackend {
	firstBranchName = MOCK_STACK_A_ID;
	secondBranchName = OTHER_HEADER_NAME;

	constructor() {
		super();
		this.stackId = MOCK_STACK_A_ID;
		this.stacks = [MOCK_STACK_A];
		this.stackDetails.set(MOCK_STACK_A_ID, MOCK_STACK_A_DETAILS);
	}
}

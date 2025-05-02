import MockBackend from '../mock/backend';
import { createMockBranchDetails, createMockStackDetails } from '../mock/stacks';
import type { Stack, StackDetails } from '$lib/stacks/stack';
import type { InvokeArgs } from '@tauri-apps/api/core';
import type { TreeChange } from '$lib/hunks/change';
import {
	createMockAdditionTreeChange,
	createMockDeletionTreeChange,
	createMockModificationTreeChange
} from '../mock/changes';

const MOCK_STACK_A_ID = 'stack-a-id';
const MOCK_STACK_B_ID = 'stack-b-id';
const MOCK_STACK_C_ID = 'stack-c-id';

const MOCK_STACK_A: Stack = {
	id: MOCK_STACK_A_ID,
	heads: [{ name: MOCK_STACK_A_ID, tip: '1234123' }],
	tip: '1234123'
};

const MOCK_BRANCH_A_CHANGES: TreeChange[] = [
	createMockAdditionTreeChange({ path: 'fileA.txt' }),
	createMockModificationTreeChange({ path: 'fileB.txt' }),
	createMockDeletionTreeChange({ path: 'fileC.txt' })
];

const MOCK_STACK_DETAILS_A = createMockStackDetails({
	derivedName: MOCK_STACK_A_ID,
	branchDetails: [createMockBranchDetails({ name: MOCK_STACK_A_ID })]
});

const MOCK_STACK_B: Stack = {
	id: MOCK_STACK_B_ID,
	heads: [{ name: MOCK_STACK_B_ID, tip: '1234123' }],
	tip: '1234123'
};

const MOCK_BRANCH_B_CHANGES: TreeChange[] = [
	createMockAdditionTreeChange({ path: 'fileD.txt' }),
	createMockModificationTreeChange({ path: 'fileE.txt' }),
	createMockDeletionTreeChange({ path: 'fileF.txt' })
];

const MOCK_STACK_DETAILS_B = createMockStackDetails({
	derivedName: MOCK_STACK_B_ID,
	branchDetails: [createMockBranchDetails({ name: MOCK_STACK_B_ID })]
});

const MOCK_STACK_C: Stack = {
	id: MOCK_STACK_C_ID,
	heads: [{ name: MOCK_STACK_C_ID, tip: '1234123' }],
	tip: '1234123'
};

const MOCK_BRANCH_C_CHANGES: TreeChange[] = [
	createMockAdditionTreeChange({ path: 'fileG.txt' }),
	createMockModificationTreeChange({ path: 'fileH.txt' }),
	createMockDeletionTreeChange({ path: 'fileI.txt' })
];

const MOCK_STACK_DETAILS_C = createMockStackDetails({
	derivedName: MOCK_STACK_C_ID,
	branchDetails: [createMockBranchDetails({ name: MOCK_STACK_C_ID })]
});

/**
 * Three branches with file changes.
 */
export default class BranchesWithChanges extends MockBackend {
	constructor() {
		super();

		this.stackId = MOCK_STACK_A_ID;

		this.stacks = [MOCK_STACK_A, MOCK_STACK_B, MOCK_STACK_C];
		this.stackDetails.set(MOCK_STACK_A_ID, MOCK_STACK_DETAILS_A);
		this.stackDetails.set(MOCK_STACK_B_ID, MOCK_STACK_DETAILS_B);
		this.stackDetails.set(MOCK_STACK_C_ID, MOCK_STACK_DETAILS_C);

		const stackAChanges = new Map<string, TreeChange[]>();
		stackAChanges.set(MOCK_STACK_A_ID, MOCK_BRANCH_A_CHANGES);

		const stackBChanges = new Map<string, TreeChange[]>();
		stackBChanges.set(MOCK_STACK_B_ID, MOCK_BRANCH_B_CHANGES);

		const stackCChanges = new Map<string, TreeChange[]>();
		stackCChanges.set(MOCK_STACK_C_ID, MOCK_BRANCH_C_CHANGES);

		this.branchChanges.set(MOCK_STACK_A_ID, stackAChanges);
		this.branchChanges.set(MOCK_STACK_B_ID, stackBChanges);
		this.branchChanges.set(MOCK_STACK_C_ID, stackCChanges);
	}
}

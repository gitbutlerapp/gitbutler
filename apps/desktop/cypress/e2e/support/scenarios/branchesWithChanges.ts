import MockBackend from '../mock/backend';
import {
	createMockAdditionTreeChange,
	createMockDeletionTreeChange,
	createMockModificationTreeChange,
	createMockUnifiedDiffPatch
} from '../mock/changes';
import { createMockBranchDetails, createMockCommit, createMockStackDetails } from '../mock/stacks';
import type { DiffDependency } from '$lib/dependencies/dependencies';
import type { TreeChange } from '$lib/hunks/change';
import type { DiffHunk } from '$lib/hunks/hunk';
import type { Stack } from '$lib/stacks/stack';

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

const MOCK_COMMIT_TITLE = 'Initial commit';
const MOCK_COMMIT_MESSAGE = 'This is a test commit';

const MOCK_COMMIT_IN_BRANCH_A = createMockCommit({
	message: `${MOCK_COMMIT_TITLE}\n\n${MOCK_COMMIT_MESSAGE}`
});

const MOCK_STACK_DETAILS_A = createMockStackDetails({
	derivedName: MOCK_STACK_A_ID,
	branchDetails: [
		createMockBranchDetails({ name: MOCK_STACK_A_ID, commits: [MOCK_COMMIT_IN_BRANCH_A] })
	]
});

const MOCK_STACK_B: Stack = {
	id: MOCK_STACK_B_ID,
	heads: [{ name: MOCK_STACK_B_ID, tip: '1234123' }],
	tip: '1234123'
};

const MOCK_FILE_D = 'fileD.txt';
const MOCK_FILE_J = 'fileJ.txt';

const MOCK_BRANCH_B_CHANGES: TreeChange[] = [
	createMockAdditionTreeChange({ path: MOCK_FILE_D }),
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

const MOCK_UNCOMMITTED_CHANGES: TreeChange[] = [
	createMockModificationTreeChange({ path: MOCK_FILE_D }), // Depends on the changes in the stack B
	createMockAdditionTreeChange({ path: MOCK_FILE_J })
];

const MOCK_FILE_D_MODIFICATION_DIFF_HUNKS: DiffHunk[] = [
	{
		oldStart: 2,
		oldLines: 8,
		newStart: 2,
		newLines: 7,
		diff: `@@ -2,8 +2,7 @@\n context line 0\n context line 1\n context line 2\n-context line 3\n-old line to be removed\n+new line added\n context line 4\n context line 5\n context line 6`
	},
	{
		oldStart: 10,
		oldLines: 7,
		newStart: 10,
		newLines: 7,
		diff: `@@ -10,7 +10,7 @@\n context before 1\n context before 2\n context before 3\n-old value\n+updated value\n context after 1\n context after 2\n context after 3`
	}
];

const MOCK_FILE_D_MODIFICATION = createMockUnifiedDiffPatch(
	MOCK_FILE_D_MODIFICATION_DIFF_HUNKS,
	2,
	3
);

const BIG_DIFF_THRESHOLD = 2501;

const MOCK_FILE_J_MODIFICATION_DIFF_HUNKS: DiffHunk[] = [
	{
		oldStart: 0,
		oldLines: 0,
		newStart: 1,
		newLines: BIG_DIFF_THRESHOLD,
		diff: `@@ -0,0 +1,${BIG_DIFF_THRESHOLD} @@\n${Array.from({ length: BIG_DIFF_THRESHOLD }, (_, i) => `+line ${i + 1}`).join('\n')}`
	}
];

const MOCK_FILE_J_MODIFICATION = createMockUnifiedDiffPatch(
	MOCK_FILE_J_MODIFICATION_DIFF_HUNKS,
	BIG_DIFF_THRESHOLD,
	0
);

const MOCK_DIFF_DEPENDENCY: DiffDependency[] = [
	[
		MOCK_FILE_D,
		{
			oldStart: 5,
			oldLines: 2,
			newStart: 5,
			newLines: 1,
			diff: `@@ -5,2 +5,1 @@\n-context line 3\n-old line to be removed\n+new line added`
		},
		[
			{
				stackId: MOCK_STACK_B_ID,
				commitId: '1234123'
			}
		]
	],
	[
		MOCK_FILE_D,
		{
			oldStart: 13,
			oldLines: 1,
			newStart: 13,
			newLines: 1,
			diff: `@@ -13,1 +13,1 @@\n-old value\n+updated value`
		},
		[
			{
				stackId: MOCK_STACK_B_ID,
				commitId: '1234123'
			}
		]
	]
];

/**
 * Three branches with file changes.
 */
export default class BranchesWithChanges extends MockBackend {
	commitTitle = MOCK_COMMIT_TITLE;
	commitMessage = MOCK_COMMIT_MESSAGE;
	dependsOnStack = MOCK_STACK_B_ID;
	bigFileName = MOCK_FILE_J;

	constructor() {
		super();

		this.stackId = MOCK_STACK_A_ID;

		this.worktreeChanges = {
			changes: MOCK_UNCOMMITTED_CHANGES,
			ignoredChanges: []
		};

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

		this.unifiedDiffs.set(MOCK_FILE_D, MOCK_FILE_D_MODIFICATION);
		this.unifiedDiffs.set(MOCK_FILE_J, MOCK_FILE_J_MODIFICATION);
		this.hunkDependencies = {
			diffs: MOCK_DIFF_DEPENDENCY,
			errors: []
		};
	}
}

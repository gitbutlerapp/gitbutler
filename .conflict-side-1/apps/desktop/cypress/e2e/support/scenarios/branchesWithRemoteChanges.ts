import MockBackend from '../mock/backend';
import {
	createMockAdditionTreeChange,
	createMockDeletionTreeChange,
	createMockModificationTreeChange,
	createMockUnifiedDiffPatch
} from '../mock/changes';
import {
	createMockBranchDetails,
	createMockCommit,
	createMockStackDetails,
	createMockUpstreamCommit,
	isIntegrateUpstreamCommitsParams
} from '../mock/stacks';
import type { DiffDependency } from '$lib/dependencies/dependencies';
import type { TreeChange } from '$lib/hunks/change';
import type { DiffHunk } from '$lib/hunks/hunk';
import type { Workspace, WorkspaceLegacy } from '@gitbutler/core/api';
import type { InvokeArgs } from '@tauri-apps/api/core';

const MOCK_STACK_A_ID = 'stack-a-id';
const MOCK_STACK_B_ID = 'stack-b-id';
const MOCK_STACK_C_ID = 'stack-c-id';

const MOCK_STACK_A: WorkspaceLegacy.StackEntry = {
	order: 0,
	id: MOCK_STACK_A_ID,
	heads: [{ name: MOCK_STACK_A_ID, tip: '1234123', isCheckedOut: true }],
	tip: '1234123',
	isCheckedOut: true
};

const MOCK_BRANCH_A_CHANGES: TreeChange[] = [
	createMockAdditionTreeChange({ path: 'fileA.txt' }),
	createMockModificationTreeChange({ path: 'fileB.txt' }),
	createMockDeletionTreeChange({ path: 'fileC.txt' })
];

const MOCK_BRANCH_A_UPSTREAM_COMMITS: Workspace.UpstreamCommit[] = [
	createMockUpstreamCommit({ id: 'upstream-commit-4', message: 'Upstream commit 4' }),
	createMockUpstreamCommit({ id: 'upstream-commit-3', message: 'Upstream commit 3' }),
	createMockUpstreamCommit({ id: 'upstream-commit-2', message: 'Upstream commit 2' }),
	createMockUpstreamCommit({ id: 'upstream-commit-1', message: 'Upstream commit 1' })
];

const MOCK_STACK_DETAILS_A = createMockStackDetails({
	derivedName: MOCK_STACK_A_ID,
	branchDetails: [
		createMockBranchDetails({
			name: MOCK_STACK_A_ID,
			commits: [
				createMockCommit({
					id: '1234123',
					message: 'Initial commit',
					state: { type: 'LocalAndRemote', subject: '1234123' }
				})
			],
			upstreamCommits: MOCK_BRANCH_A_UPSTREAM_COMMITS
		})
	]
});

const MOCK_STACK_B: WorkspaceLegacy.StackEntry = {
	order: 1,
	id: MOCK_STACK_B_ID,
	heads: [{ name: MOCK_STACK_B_ID, tip: '1234123', isCheckedOut: true }],
	tip: '1234123',
	isCheckedOut: true
};

const MOCK_FILE_D = 'fileD.txt';

const MOCK_BRANCH_B_CHANGES: TreeChange[] = [
	createMockAdditionTreeChange({ path: MOCK_FILE_D }),
	createMockModificationTreeChange({ path: 'fileE.txt' }),
	createMockDeletionTreeChange({ path: 'fileF.txt' })
];

const MOCK_STACK_B_OTHER_BRANCH_NAME = 'other-branch-name';

const MOCK_STACK_DETAILS_B = createMockStackDetails({
	derivedName: MOCK_STACK_B_ID,
	branchDetails: [
		createMockBranchDetails({ name: MOCK_STACK_B_ID }),
		createMockBranchDetails({ name: MOCK_STACK_B_OTHER_BRANCH_NAME })
	]
});

const MOCK_STACK_C: WorkspaceLegacy.StackEntry = {
	order: 2,
	id: MOCK_STACK_C_ID,
	heads: [{ name: MOCK_STACK_C_ID, tip: '1234123', isCheckedOut: true }],
	tip: '1234123',
	isCheckedOut: true
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
	createMockModificationTreeChange({ path: MOCK_FILE_D }) // Depends on the changes in the stack B
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
export default class BranchesWithRemoteChanges extends MockBackend {
	localOnlyBranchStackId = MOCK_STACK_B_ID;
	constructor() {
		super();

		this.stackId = MOCK_STACK_A_ID;

		this.worktreeChanges = {
			changes: MOCK_UNCOMMITTED_CHANGES,
			ignoredChanges: [],
			assignments: [],
			assignmentsError: null,
			dependencies: {
				diffs: MOCK_DIFF_DEPENDENCY,
				errors: []
			},
			dependenciesError: null
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
	}

	public integrateUpstreamCommits(args: InvokeArgs | undefined) {
		if (!args || !isIntegrateUpstreamCommitsParams(args)) {
			throw new Error('Invalid arguments for integrateUpstreamCommits');
		}

		const { stackId, seriesName } = args;
		const stackDetails = this.stackDetails.get(stackId);
		if (!stackDetails) {
			throw new Error(`Stack details not found for stack ID: ${stackId}`);
		}
		const editableDetails = structuredClone(stackDetails);

		const branchDetails = editableDetails.branchDetails.find(
			(branch) => branch.name === seriesName
		);

		if (!branchDetails) {
			throw new Error(`Branch details not found for branch name: ${seriesName}`);
		}

		const upstreamCommits = branchDetails.upstreamCommits;

		for (const upstreamCommit of upstreamCommits) {
			branchDetails.commits.splice(
				0,
				0,
				createMockCommit({
					...upstreamCommit,
					state: {
						type: 'LocalAndRemote',
						subject: upstreamCommit.id
					}
				})
			);
		}

		branchDetails.upstreamCommits = [];
		this.stackDetails.set(stackId, editableDetails);
	}
}

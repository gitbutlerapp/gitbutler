import MockBackend from '../mock/backend';
import {
	createMockAdditionTreeChange,
	createMockDeletionTreeChange,
	createMockModificationTreeChange,
	createMockUnifiedDiffPatch
} from '../mock/changes';
import { getMockTemplateContent } from '../mock/review';
import { createMockBranchDetails, createMockCommit, createMockStackDetails } from '../mock/stacks';
import type { DiffDependency } from '$lib/dependencies/dependencies';
import type { TreeChange } from '$lib/hunks/change';
import type { DiffHunk } from '$lib/hunks/hunk';
import type { Stack } from '$lib/stacks/stack';
import type { CreateCommitOutcome } from '$lib/stacks/stackService.svelte';

const MOCK_STACK_A_ID = 'stack-a-id';
const MOCK_STACK_B_ID = 'stack-b-id';
const MOCK_STACK_C_ID = 'stack-c-id';

const MOCK_FILE_A = 'fileA.ts';
const MOCK_STACK_A: Stack = {
	order: 0,
	id: MOCK_STACK_A_ID,
	heads: [{ name: MOCK_STACK_A_ID, tip: '1234123' }],
	tip: '1234123'
};

const MOCK_BRANCH_A_CHANGES: TreeChange[] = [
	createMockAdditionTreeChange({ path: MOCK_FILE_A }),
	createMockModificationTreeChange({ path: 'fileB.txt' }),
	createMockDeletionTreeChange({ path: 'fileC.txt' })
];

const MOCK_COMMIT_TITLE_A = 'Initial commit';
const MOCK_COMMIT_MESSAGE_A = 'This is a test commit';

const MOCK_COMMIT_IN_BRANCH_A = createMockCommit({
	id: '444444',
	message: `${MOCK_COMMIT_TITLE_A}\n\n${MOCK_COMMIT_MESSAGE_A}`
});

const MOCK_STACK_DETAILS_A = createMockStackDetails({
	derivedName: MOCK_STACK_A_ID,
	branchDetails: [
		createMockBranchDetails({ name: MOCK_STACK_A_ID, commits: [MOCK_COMMIT_IN_BRANCH_A] })
	]
});

const MOCK_STACK_B: Stack = {
	order: 1,
	id: MOCK_STACK_B_ID,
	heads: [{ name: MOCK_STACK_B_ID, tip: '1234123' }],
	tip: '1234123'
};

const MOCK_FILE_D = 'fileD.txt';
const MOCK_FILE_J = 'fileJ.txt';

const MOCK_COMMIT_B_CHANGES: TreeChange[] = [createMockAdditionTreeChange({ path: MOCK_FILE_D })];

const MOCK_COMMIT_B_CHANGES_2: TreeChange[] = [
	createMockModificationTreeChange({ path: 'fileE.txt' }),
	createMockDeletionTreeChange({ path: 'fileF.txt' })
];

const MOCK_BRANCH_B_CHANGES: TreeChange[] = [...MOCK_COMMIT_B_CHANGES, ...MOCK_COMMIT_B_CHANGES_2];

const MOCK_COMMIT_TITLE_B = 'Second commit';
const MOCK_COMMIT_MESSAGE_B = 'This is another test commit';
const MOCK_COMMIT_IN_BRANCH_B = createMockCommit({
	message: `${MOCK_COMMIT_TITLE_B}\n\n${MOCK_COMMIT_MESSAGE_B}`
});

const MOCK_COMMIT_TITLE_B_2 = 'Also second commit';
const MOCK_COMMIT_MESSAGE_B_2 = 'This is another test commit, but with a different title';
const MOCK_COMMIT_IN_BRANCH_B_2 = createMockCommit({
	id: '1234123-2',
	message: `${MOCK_COMMIT_TITLE_B_2}\n\n${MOCK_COMMIT_MESSAGE_B_2}`
});

const MOCK_STACK_DETAILS_B = createMockStackDetails({
	derivedName: MOCK_STACK_B_ID,
	branchDetails: [
		createMockBranchDetails({
			name: MOCK_STACK_B_ID,
			commits: [MOCK_COMMIT_IN_BRANCH_B, MOCK_COMMIT_IN_BRANCH_B_2]
		})
	]
});

const MOCK_STACK_C: Stack = {
	order: 2,
	id: MOCK_STACK_C_ID,
	heads: [{ name: MOCK_STACK_C_ID, tip: '1234123' }],
	tip: '1234123'
};

const MOCK_BRANCH_C_CHANGES: TreeChange[] = [
	createMockAdditionTreeChange({ path: 'fileG.txt' }),
	createMockModificationTreeChange({ path: 'fileH.txt' }),
	createMockDeletionTreeChange({ path: 'fileI.txt' })
];

const MOCK_COMMIT_TITLE_C = 'Third commit';
const MOCK_COMMIT_MESSAGE_C = 'This is yet another test commit';
const MOCK_COMMIT_IN_BRANCH_C = createMockCommit({
	message: `${MOCK_COMMIT_TITLE_C}\n\n${MOCK_COMMIT_MESSAGE_C}`
});

const MOCK_STACK_DETAILS_C = createMockStackDetails({
	derivedName: MOCK_STACK_C_ID,
	branchDetails: [
		createMockBranchDetails({ name: MOCK_STACK_C_ID, commits: [MOCK_COMMIT_IN_BRANCH_C] })
	]
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

const MOCK_FILE_1 = 'file1.txt';
const MOCK_FILE_2 = 'file2.txt';
const MOCK_FILE_3 = 'file3.txt';
const MOCK_FILE_4 = 'file4.txt';
const MOCK_FILE_5 = 'file5.txt';
const MOCK_FILE_6 = 'file6.txt';

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
	],
	[
		MOCK_FILE_4,
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
				commitId: 'asdfasdfadfasdfadfasdfs'
			},
			{
				stackId: MOCK_STACK_B_ID,
				commitId: '5545454545fafafafafa234'
			},
			{
				stackId: MOCK_STACK_C_ID,
				commitId: '8s9d8s9df87s9df87s9dfss'
			}
		]
	],
	[
		MOCK_FILE_5,
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
	],
	[
		MOCK_FILE_6,
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

const MOCK_FILE_A_MODIFICATION_DIFF_HUNKS: DiffHunk[] = [
	{
		oldStart: 1,
		oldLines: 7,
		newStart: 1,
		newLines: 12,
		diff: "@@ -1,7 +1,12 @@\n+// Importing StackOrder type for branch ordering operations\n import { StackOrder } from '$lib/branches/branch';\n+// Importing showToast for displaying notifications to the user\n import { showToast } from '$lib/notifications/toasts';\n+// Importing ClientState and BackendApi for API endpoint injection and state management\n import { ClientState, type BackendApi } from '$lib/state/clientState.svelte';\n+// Importing custom selectors for entity selection by IDs or index\n import { createSelectByIds, createSelectNth } from '$lib/state/customSelectors';\n+// Importing Redux tag helpers for cache invalidation and entity tagging\n import {\n \tinvalidatesItem,\n \tinvalidatesList,\n"
	},
	{
		oldStart: 9,
		oldLines: 8,
		newStart: 14,
		newLines: 11,
		diff: "@@ -9,8 +14,11 @@\n \tprovidesList,\n \tReduxTag\n } from '$lib/state/tags';\n+// Utility to split commit messages into title and description\n import { splitMessage } from '$lib/utils/commitMessage';\n+// Redux Toolkit helpers for entity state management\n import { createEntityAdapter, type EntityState } from '@reduxjs/toolkit';\n+// Types for backend API and domain models\n import type { TauriCommandError } from '$lib/backend/ipc';\n import type { Commit, CommitDetails, UpstreamCommit } from '$lib/branches/v3';\n import type { CommitKey } from '$lib/commits/commit';\n"
	},
	{
		oldStart: 22,
		oldLines: 6,
		newStart: 30,
		newLines: 7,
		diff: "@@ -22,6 +30,7 @@\n import type { UiState } from '$lib/state/uiState.svelte';\n import type { User } from '$lib/user/user';\n \n+// Parameters for creating or updating a branch\n type BranchParams = {\n \tname?: string;\n \townership?: string;\n"
	},
	{
		oldStart: 31,
		oldLines: 6,
		newStart: 40,
		newLines: 7,
		diff: '@@ -31,6 +40,7 @@\n \tselected_for_changes?: boolean;\n };\n \n+// Request type for creating a commit from worktree changes\n export type CreateCommitRequest = {\n \tstackId: string;\n \tmessage: string;\n'
	},
	{
		oldStart: 44,
		oldLines: 16,
		newStart: 54,
		newLines: 20,
		diff: "@@ -44,16 +54,20 @@\n \t}[];\n };\n \n+// Type for a single worktree change in a commit request\n export type CreateCommitRequestWorktreeChanges = CreateCommitRequest['worktreeChanges'][number];\n \n+// Supported stack actions for error handling\n type StackAction = 'push';\n \n+// Error info structure for stack actions\n type StackErrorInfo = {\n \ttitle: string;\n \tcodeInfo: Record<string, string>;\n \tdefaultInfo: string;\n };\n \n+// Error info mapping for stack actions\n const ERROR_INFO: Record<StackAction, StackErrorInfo> = {\n \tpush: {\n \t\ttitle: 'Git push failed',\n"
	},
	{
		oldStart: 64,
		oldLines: 6,
		newStart: 78,
		newLines: 13,
		diff: '@@ -64,6 +78,13 @@\n \t}\n };\n \n+/**\n+ * Surfaces stack-related errors to the user via toast notifications.\n+ * @param action The stack action that failed\n+ * @param errorCode The error code returned by the backend\n+ * @param errorMessage The error message returned by the backend\n+ * @returns true if an error was surfaced, false otherwise\n+ */\n function surfaceStackError(action: StackAction, errorCode: string, errorMessage: string): boolean {\n \tconst reason = ERROR_INFO[action].codeInfo[errorCode] ?? ERROR_INFO[action].defaultInfo;\n \tconst title = ERROR_INFO[action].title;\n'
	},
	{
		oldStart: 86,
		oldLines: 14,
		newStart: 107,
		newLines: 18,
		diff: "@@ -86,14 +107,18 @@\n \t}\n }\n \n+// Union type for identifying a commit or a change\n export type CommitIdOrChangeId = { CommitId: string } | { ChangeId: string };\n+// Strategies for integrating upstream commits into a series\n export type SeriesIntegrationStrategy = 'merge' | 'rebase' | 'hardreset';\n \n+// Result type for a branch push operation\n export interface BranchPushResult {\n \trefname: string;\n \tremote: string;\n }\n \n+// Reasons why a change might be rejected during commit creation\n type RejectionReason =\n \t| 'NoEffectiveChanges'\n \t| 'CherryPickMergeConflict'\n"
	},
	{
		oldStart: 105,
		oldLines: 14,
		newStart: 130,
		newLines: 26,
		diff: "@@ -105,14 +130,26 @@\n \t| 'UnsupportedTreeEntry'\n \t| 'MissingDiffSpecAssociation';\n \n+// Outcome of a create commit operation, including new commit ID and rejected changes\n export type CreateCommitOutcome = {\n \tnewCommit: string;\n \tpathsToRejectedChanges: [RejectionReason, string][];\n };\n \n+/**\n+ * Service class for interacting with stack and branch-related backend APIs.\n+ * Provides methods for querying, mutating, and managing stacks, branches, and commits.\n+ */\n export class StackService {\n+\t// API endpoints injected from the backend client state\n \tprivate api: ReturnType<typeof injectEndpoints>;\n \n+\t/**\n+\t * Constructs a StackService instance.\n+\t * @param backendApi The backend API instance\n+\t * @param forgeFactory The forge factory for cache invalidation\n+\t * @param uiState The UI state for managing local UI state\n+\t */\n \tconstructor(\n \t\tbackendApi: BackendApi,\n \t\tprivate forgeFactory: DefaultForgeFactory,\n"
	},
	{
		oldStart: 121,
		oldLines: 6,
		newStart: 158,
		newLines: 9,
		diff: '@@ -121,6 +158,9 @@\n \t\tthis.api = injectEndpoints(backendApi);\n \t}\n \n+\t/**\n+\t * Returns a query for all stacks in a project.\n+\t */\n \tstacks(projectId: string) {\n \t\treturn this.api.endpoints.stacks.useQuery(\n \t\t\t{ projectId },\n'
	},
	{
		oldStart: 130,
		oldLines: 6,
		newStart: 170,
		newLines: 9,
		diff: '@@ -130,6 +170,9 @@\n \t\t);\n \t}\n \n+\t/**\n+\t * Fetches all stacks in a project (async version).\n+\t */\n \tasync fetchStacks(projectId: string) {\n \t\treturn await this.api.endpoints.stacks.fetch(\n \t\t\t{ projectId },\n'
	},
	{
		oldStart: 139,
		oldLines: 6,
		newStart: 182,
		newLines: 9,
		diff: '@@ -139,6 +182,9 @@\n \t\t);\n \t}\n \n+\t/**\n+\t * Returns a query for the stack at a given index in a project.\n+\t */\n \tstackAt(projectId: string, index: number) {\n \t\treturn this.api.endpoints.stacks.useQuery(\n \t\t\t{ projectId },\n'
	},
	{
		oldStart: 148,
		oldLines: 6,
		newStart: 194,
		newLines: 9,
		diff: '@@ -148,6 +194,9 @@\n \t\t);\n \t}\n \n+\t/**\n+\t * Returns a query for a stack by its ID in a project.\n+\t */\n \tstackById(projectId: string, id: string) {\n \t\treturn this.api.endpoints.stacks.useQuery(\n \t\t\t{ projectId },\n'
	},
	{
		oldStart: 157,
		oldLines: 6,
		newStart: 206,
		newLines: 9,
		diff: '@@ -157,6 +206,9 @@\n \t\t);\n \t}\n \n+\t/**\n+\t * Returns a query for all stacks in a project, including archived or hidden ones.\n+\t */\n \tallStacks(projectId: string) {\n \t\treturn this.api.endpoints.allStacks.useQuery(\n \t\t\t{ projectId },\n'
	},
	{
		oldStart: 166,
		oldLines: 6,
		newStart: 218,
		newLines: 9,
		diff: '@@ -166,6 +218,9 @@\n \t\t);\n \t}\n \n+\t/**\n+\t * Returns a query for the nth stack in all stacks (including archived/hidden).\n+\t */\n \tallStackAt(projectId: string, index: number) {\n \t\treturn this.api.endpoints.allStacks.useQuery(\n \t\t\t{ projectId },\n'
	},
	{
		oldStart: 175,
		oldLines: 6,
		newStart: 230,
		newLines: 9,
		diff: '@@ -175,6 +230,9 @@\n \t\t);\n \t}\n \n+\t/**\n+\t * Returns a query for a stack by ID from all stacks (including archived/hidden).\n+\t */\n \tallStackById(projectId: string, id: string) {\n \t\treturn this.api.endpoints.allStacks.useQuery(\n \t\t\t{ projectId },\n'
	},
	{
		oldStart: 184,
		oldLines: 6,
		newStart: 242,
		newLines: 9,
		diff: '@@ -184,6 +242,9 @@\n \t\t);\n \t}\n \n+\t/**\n+\t * Returns a query for the default branch name of a stack.\n+\t */\n \tdefaultBranch(projectId: string, stackId: string) {\n \t\treturn this.api.endpoints.stacks.useQuery(\n \t\t\t{ projectId },\n'
	},
	{
		oldStart: 193,
		oldLines: 6,
		newStart: 254,
		newLines: 9,
		diff: '@@ -193,6 +254,9 @@\n \t\t);\n \t}\n \n+\t/**\n+\t * Returns a query for stack details/info by stack ID.\n+\t */\n \tstackInfo(projectId: string, stackId: string) {\n \t\treturn this.api.endpoints.stackDetails.useQuery(\n \t\t\t{ projectId, stackId },\n'
	},
	{
		oldStart: 200,
		oldLines: 6,
		newStart: 264,
		newLines: 9,
		diff: '@@ -200,6 +264,9 @@\n \t\t);\n \t}\n \n+\t/**\n+\t * Returns a query for branch details by branch name in a stack.\n+\t */\n \tbranchDetails(projectId: string, stackId: string, branchName: string) {\n \t\treturn this.api.endpoints.stackDetails.useQuery(\n \t\t\t{ projectId, stackId },\n'
	},
	{
		oldStart: 210,
		oldLines: 22,
		newStart: 277,
		newLines: 37,
		diff: '@@ -210,22 +277,37 @@\n \t\t);\n \t}\n \n+\t/**\n+\t * Returns a mutation for creating a new stack.\n+\t */\n \tget newStack() {\n \t\treturn this.api.endpoints.createStack.useMutation();\n \t}\n \n+\t/**\n+\t * Returns the mutation function for creating a new stack.\n+\t */\n \tget newStackMutation() {\n \t\treturn this.api.endpoints.createStack.mutate;\n \t}\n \n+\t/**\n+\t * Returns the mutation function for updating a stack.\n+\t */\n \tget updateStack() {\n \t\treturn this.api.endpoints.updateStack.mutate;\n \t}\n \n+\t/**\n+\t * Returns the mutation function for updating branch order in a stack.\n+\t */\n \tget updateStackOrder() {\n \t\treturn this.api.endpoints.updateStackOrder.mutate;\n \t}\n \n+\t/**\n+\t * Returns a query for all branches in a stack.\n+\t */\n \tbranches(projectId: string, stackId: string) {\n \t\treturn this.api.endpoints.stackDetails.useQuery(\n \t\t\t{ projectId, stackId },\n'
	},
	{
		oldStart: 235,
		oldLines: 6,
		newStart: 317,
		newLines: 9,
		diff: '@@ -235,6 +317,9 @@\n \t\t);\n \t}\n \n+\t/**\n+\t * Returns a query for the branch at a given index in a stack.\n+\t */\n \tbranchAt(projectId: string, stackId: string, index: number) {\n \t\treturn this.api.endpoints.stackDetails.useQuery(\n \t\t\t{ projectId, stackId },\n'
	},
	{
		oldStart: 245,
		oldLines: 6,
		newStart: 330,
		newLines: 9,
		diff: '@@ -245,6 +330,9 @@\n \t}\n \n \t/** Returns the parent of the branch specified by the provided name */\n+\t/**\n+\t * Returns a query for the parent branch of a given branch by name in a stack.\n+\t */\n \tbranchParentByName(projectId: string, stackId: string, name: string) {\n \t\treturn this.api.endpoints.stackDetails.useQuery(\n \t\t\t{ projectId, stackId },\n'
	},
	{
		oldStart: 263,
		oldLines: 6,
		newStart: 351,
		newLines: 9,
		diff: '@@ -263,6 +351,9 @@\n \t\t);\n \t}\n \t/** Returns the child of the branch specified by the provided name */\n+\t/**\n+\t * Returns a query for the child branch of a given branch by name in a stack.\n+\t */\n \tbranchChildByName(projectId: string, stackId: string, name: string) {\n \t\treturn this.api.endpoints.stackDetails.useQuery(\n \t\t\t{ projectId, stackId },\n'
	},
	{
		oldStart: 281,
		oldLines: 6,
		newStart: 372,
		newLines: 9,
		diff: '@@ -281,6 +372,9 @@\n \t\t);\n \t}\n \n+\t/**\n+\t * Returns a query for a branch by name in a stack.\n+\t */\n \tbranchByName(projectId: string, stackId: string, name: string) {\n \t\treturn this.api.endpoints.stackDetails.useQuery(\n \t\t\t{ projectId, stackId },\n'
	},
	{
		oldStart: 288,
		oldLines: 6,
		newStart: 382,
		newLines: 9,
		diff: '@@ -288,6 +382,9 @@\n \t\t);\n \t}\n \n+\t/**\n+\t * Returns a query for all commits in a branch of a stack.\n+\t */\n \tcommits(projectId: string, stackId: string, branchName: string) {\n \t\treturn this.api.endpoints.stackDetails.useQuery(\n \t\t\t{ projectId, stackId },\n'
	},
	{
		oldStart: 298,
		oldLines: 6,
		newStart: 395,
		newLines: 9,
		diff: '@@ -298,6 +395,9 @@\n \t\t);\n \t}\n \n+\t/**\n+\t * Returns a query for the commit at a given index in a branch of a stack.\n+\t */\n \tcommitAt(projectId: string, stackId: string, branchName: string, index: number) {\n \t\treturn this.api.endpoints.stackDetails.useQuery(\n \t\t\t{ projectId, stackId },\n'
	},
	{
		oldStart: 308,
		oldLines: 6,
		newStart: 408,
		newLines: 9,
		diff: '@@ -308,6 +408,9 @@\n \t\t);\n \t}\n \n+\t/**\n+\t * Returns a query for a commit by its key (stackId and commitId).\n+\t */\n \tcommitById(projectId: string, commitKey: CommitKey) {\n \t\tconst { stackId, commitId } = commitKey;\n \t\treturn this.api.endpoints.stackDetails.useQuery(\n'
	},
	{
		oldStart: 318,
		oldLines: 6,
		newStart: 421,
		newLines: 9,
		diff: '@@ -318,6 +421,9 @@\n \t\t);\n \t}\n \n+\t/**\n+\t * Returns a query for all upstream commits in a branch of a stack.\n+\t */\n \tupstreamCommits(projectId: string, stackId: string, branchName: string) {\n \t\treturn this.api.endpoints.stackDetails.useQuery(\n \t\t\t{ projectId, stackId },\n'
	},
	{
		oldStart: 328,
		oldLines: 6,
		newStart: 434,
		newLines: 9,
		diff: '@@ -328,6 +434,9 @@\n \t\t);\n \t}\n \n+\t/**\n+\t * Returns a query for the upstream commit at a given index in a branch of a stack.\n+\t */\n \tupstreamCommitAt(projectId: string, stackId: string, branchName: string, index: number) {\n \t\treturn this.api.endpoints.stackDetails.useQuery(\n \t\t\t{ projectId, stackId },\n'
	},
	{
		oldStart: 339,
		oldLines: 6,
		newStart: 448,
		newLines: 9,
		diff: '@@ -339,6 +448,9 @@\n \t\t);\n \t}\n \n+\t/**\n+\t * Returns a query for an upstream commit by its key (stackId and commitId).\n+\t */\n \tupstreamCommitById(projectId: string, commitKey: CommitKey) {\n \t\tconst { stackId, commitId } = commitKey;\n \t\treturn this.api.endpoints.stackDetails.useQuery(\n'
	},
	{
		oldStart: 350,
		oldLines: 6,
		newStart: 462,
		newLines: 10,
		diff: '@@ -350,6 +462,10 @@\n \t\t);\n \t}\n \n+\t/**\n+\t * Returns a mutation for pushing a stack to the remote.\n+\t * Handles cache invalidation and error surfacing.\n+\t */\n \tget pushStack() {\n \t\treturn this.api.endpoints.pushStack.useMutation({\n \t\t\tsideEffect: (_, args) => {\n'
	},
	{
		oldStart: 369,
		oldLines: 14,
		newStart: 485,
		newLines: 23,
		diff: '@@ -369,14 +485,23 @@\n \t\t});\n \t}\n \n+\t/**\n+\t * Returns a mutation for creating a commit from worktree changes.\n+\t */\n \tget createCommit() {\n \t\treturn this.api.endpoints.createCommit.useMutation();\n \t}\n \n+\t/**\n+\t * Returns the legacy mutation function for creating a commit.\n+\t */\n \tget createCommitLegacy() {\n \t\treturn this.api.endpoints.createCommitLegacy.mutate;\n \t}\n \n+\t/**\n+\t * Returns a query for all changes in a commit.\n+\t */\n \tcommitChanges(projectId: string, commitId: string) {\n \t\treturn this.api.endpoints.commitDetails.useQuery(\n \t\t\t{ projectId, commitId },\n'
	},
	{
		oldStart: 384,
		oldLines: 6,
		newStart: 509,
		newLines: 9,
		diff: '@@ -384,6 +509,9 @@\n \t\t);\n \t}\n \n+\t/**\n+\t * Returns a query for a specific change in a commit by file path.\n+\t */\n \tcommitChange(projectId: string, commitId: string, path: string) {\n \t\treturn this.api.endpoints.commitDetails.useQuery(\n \t\t\t{ projectId, commitId },\n'
	},
	{
		oldStart: 391,
		oldLines: 6,
		newStart: 519,
		newLines: 9,
		diff: '@@ -391,6 +519,9 @@\n \t\t);\n \t}\n \n+\t/**\n+\t * Returns a query for specific changes in a commit by file paths.\n+\t */\n \tcommitChangesByPaths(projectId: string, commitId: string, paths: string[]) {\n \t\treturn this.api.endpoints.commitDetails.useQuery(\n \t\t\t{ projectId, commitId },\n'
	},
	{
		oldStart: 398,
		oldLines: 6,
		newStart: 529,
		newLines: 9,
		diff: '@@ -398,6 +529,9 @@\n \t\t);\n \t}\n \n+\t/**\n+\t * Returns a query for commit details by commit ID.\n+\t */\n \tcommitDetails(projectId: string, commitId: string) {\n \t\treturn this.api.endpoints.commitDetails.useQuery(\n \t\t\t{ projectId, commitId },\n'
	},
	{
		oldStart: 410,
		oldLines: 6,
		newStart: 544,
		newLines: 9,
		diff: '@@ -410,6 +544,9 @@\n \t * If the branch is part of a stack and if the stackId is provided, this will include only the changes up to the next branch in the stack.\n \t * Otherwise, if stackId is not provided, this will include all changes as compared to the target branch\n \t */\n+\t/**\n+\t * Returns a query for all changes in a branch, optionally limited to a stack or remote.\n+\t */\n \tbranchChanges(args: {\n \t\tprojectId: string;\n \t\tstackId?: string;\n'
	},
	{
		oldStart: 427,
		oldLines: 6,
		newStart: 564,
		newLines: 9,
		diff: '@@ -427,6 +564,9 @@\n \t\t);\n \t}\n \n+\t/**\n+\t * Returns a query for a specific change in a branch by file path.\n+\t */\n \tbranchChange(args: {\n \t\tprojectId: string;\n \t\tstackId?: string;\n'
	},
	{
		oldStart: 445,
		oldLines: 6,
		newStart: 585,
		newLines: 9,
		diff: '@@ -445,6 +585,9 @@\n \t\t);\n \t}\n \n+\t/**\n+\t * Returns a query for specific changes in a branch by file paths.\n+\t */\n \tbranchChangesByPaths(args: {\n \t\tprojectId: string;\n \t\tstackId?: string;\n'
	},
	{
		oldStart: 463,
		oldLines: 14,
		newStart: 606,
		newLines: 23,
		diff: '@@ -463,14 +606,23 @@\n \t\t);\n \t}\n \n+\t/**\n+\t * Returns a mutation for updating a commit message.\n+\t */\n \tget updateCommitMessage() {\n \t\treturn this.api.endpoints.updateCommitMessage.useMutation();\n \t}\n \n+\t/**\n+\t * Returns a mutation for creating a new branch in a stack.\n+\t */\n \tget newBranch() {\n \t\treturn this.api.endpoints.newBranch.useMutation();\n \t}\n \n+\t/**\n+\t * Uncommits the latest commit in a branch and updates the UI state with the commit message.\n+\t */\n \tasync uncommit(args: {\n \t\tprojectId: string;\n \t\tstackId: string;\n'
	},
	{
		oldStart: 494,
		oldLines: 34,
		newStart: 646,
		newLines: 58,
		diff: '@@ -494,34 +646,58 @@\n \t\treturn await this.api.endpoints.uncommit.mutate(args);\n \t}\n \n+\t/**\n+\t * Returns a mutation for inserting a blank commit at a specific position.\n+\t */\n \tget insertBlankCommit() {\n \t\treturn this.api.endpoints.insertBlankCommit.useMutation();\n \t}\n \n+\t/**\n+\t * Returns the mutation function for unapplied stacks.\n+\t */\n \tget unapply() {\n \t\treturn this.api.endpoints.unapply.mutate;\n \t}\n \n+\t/**\n+\t * Returns a mutation for publishing a branch (push to review).\n+\t */\n \tget publishBranch() {\n \t\treturn this.api.endpoints.publishBranch.useMutation();\n \t}\n \n+\t/**\n+\t * Returns the mutation function for discarding worktree changes.\n+\t */\n \tget discardChanges() {\n \t\treturn this.api.endpoints.discardChanges.mutate;\n \t}\n \n+\t/**\n+\t * Returns the mutation function for moving changes between commits.\n+\t */\n \tget moveChangesBetweenCommits() {\n \t\treturn this.api.endpoints.moveChangesBetweenCommits.mutate;\n \t}\n \n+\t/**\n+\t * Returns the mutation function for stashing changes into a branch.\n+\t */\n \tget stashIntoBranch() {\n \t\treturn this.api.endpoints.stashIntoBranch.mutate;\n \t}\n \n+\t/**\n+\t * Returns a mutation for updating the PR number of a branch.\n+\t */\n \tget updateBranchPrNumber() {\n \t\treturn this.api.endpoints.updateBranchPrNumber.useMutation();\n \t}\n \n+\t/**\n+\t * Returns a mutation for updating the name of a branch, with UI state side effects.\n+\t */\n \tget updateBranchName() {\n \t\treturn this.api.endpoints.updateBranchName.useMutation({\n \t\t\tpreEffect: (args) => {\n'
	},
	{
		oldStart: 543,
		oldLines: 67,
		newStart: 719,
		newLines: 118,
		diff: '@@ -543,67 +719,118 @@\n \t\t});\n \t}\n \n+\t/**\n+\t * Returns a mutation for removing a branch from a stack.\n+\t */\n \tget removeBranch() {\n \t\treturn this.api.endpoints.removeBranch.useMutation();\n \t}\n \n+\t/**\n+\t * Returns a mutation for updating the description of a branch.\n+\t */\n \tget updateBranchDescription() {\n \t\treturn this.api.endpoints.updateBranchDescription.useMutation();\n \t}\n \n+\t/**\n+\t * Returns the mutation function for reordering branches in a stack.\n+\t */\n \tget reorderStack() {\n \t\treturn this.api.endpoints.reorderStack.mutate;\n \t}\n \n+\t/**\n+\t * Returns the mutation function for moving a commit between stacks.\n+\t */\n \tget moveCommit() {\n \t\treturn this.api.endpoints.moveCommit.mutate;\n \t}\n \n+\t/**\n+\t * Returns a mutation for integrating upstream commits into a stack.\n+\t */\n \tget integrateUpstreamCommits() {\n \t\treturn this.api.endpoints.integrateUpstreamCommits.useMutation();\n \t}\n \n+\t/**\n+\t * Returns the mutation function for unapplied lines in a hunk (legacy).\n+\t */\n \tget legacyUnapplyLines() {\n \t\treturn this.api.endpoints.legacyUnapplyLines.mutate;\n \t}\n \n+\t/**\n+\t * Returns the mutation function for unapplied a hunk (legacy).\n+\t */\n \tget legacyUnapplyHunk() {\n \t\treturn this.api.endpoints.legacyUnapplyHunk.mutate;\n \t}\n \n+\t/**\n+\t * Returns the mutation function for unapplied files (legacy).\n+\t */\n \tget legacyUnapplyFiles() {\n \t\treturn this.api.endpoints.legacyUnapplyFiles.mutate;\n \t}\n \n+\t/**\n+\t * Returns the mutation function for updating branch ownership (legacy).\n+\t */\n \tget legacyUpdateBranchOwnership() {\n \t\treturn this.api.endpoints.legacyUpdateBranchOwnership.mutate;\n \t}\n \n+\t/**\n+\t * Returns the mutation function for creating a virtual branch from an existing branch.\n+\t */\n \tget createVirtualBranchFromBranch() {\n \t\treturn this.api.endpoints.createVirtualBranchFromBranch.mutate;\n \t}\n \n+\t/**\n+\t * Returns the mutation function for deleting a local branch.\n+\t */\n \tget deleteLocalBranch() {\n \t\treturn this.api.endpoints.deleteLocalBranch.mutate;\n \t}\n \n+\t/**\n+\t * Returns the mutation function for squashing multiple commits into one.\n+\t */\n \tget squashCommits() {\n \t\treturn this.api.endpoints.squashCommits.mutate;\n \t}\n \n+\t/**\n+\t * Returns a mutation for amending a commit with new changes.\n+\t */\n \tget amendCommit() {\n \t\treturn this.api.endpoints.amendCommit.useMutation();\n \t}\n \n+\t/**\n+\t * Returns the mutation function for amending a commit (direct function).\n+\t */\n \tget amendCommitMutation() {\n \t\treturn this.api.endpoints.amendCommit.mutate;\n \t}\n \n+\t/**\n+\t * Returns the mutation function for moving a file between commits.\n+\t */\n \tget moveCommitFileMutation() {\n \t\treturn this.api.endpoints.moveCommitFile.mutate;\n \t}\n \n \t/** Squash all the commits in a branch together */\n+\t/**\n+\t * Squashes all local (non-integrated) commits in a branch into the last commit.\n+\t * @param projectId The project ID\n+\t * @param stackId The stack ID\n+\t * @param branchName The branch name\n+\t */\n \tasync squashAllCommits({\n \t\tprojectId,\n \t\tstackId,\n'
	},
	{
		oldStart: 639,
		oldLines: 10,
		newStart: 866,
		newLines: 16,
		diff: '@@ -639,10 +866,16 @@\n \t\t});\n \t}\n \n+\t/**\n+\t * Fetches a new branch name suggestion from the backend.\n+\t */\n \tasync newBranchName(projectId: string) {\n \t\treturn await this.api.endpoints.newBranchName.fetch({ projectId }, { forceRefetch: true });\n \t}\n \n+\t/**\n+\t * Normalizes a branch name using backend rules.\n+\t */\n \tasync normalizeBranchName(name: string) {\n \t\treturn await this.api.endpoints.normalizeBranchName.fetch({ name }, { forceRefetch: true });\n \t}\n'
	},
	{
		oldStart: 650,
		oldLines: 6,
		newStart: 883,
		newLines: 9,
		diff: '@@ -650,6 +883,9 @@\n \t/**\n \t * Note: This is specifically for looking up branches outside of\n \t * a stacking context. You almost certainly want `stackDetails`\n+\t */\n+\t/**\n+\t * Returns a query for branch details outside of a stacking context.\n \t */\n \tunstackedBranchDetails(projectId: string, branchName: string, remote?: string) {\n \t\treturn this.api.endpoints.unstackedBranchDetails.useQuery(\n'
	},
	{
		oldStart: 658,
		oldLines: 6,
		newStart: 894,
		newLines: 9,
		diff: '@@ -658,6 +894,9 @@\n \t\t);\n \t}\n \n+\t/**\n+\t * Returns a query for all commits in a branch outside of a stacking context.\n+\t */\n \tunstackedCommits(projectId: string, branchName: string, remote?: string) {\n \t\treturn this.api.endpoints.unstackedBranchDetails.useQuery(\n \t\t\t{ projectId, branchName, remote },\n'
	},
	{
		oldStart: 667,
		oldLines: 6,
		newStart: 906,
		newLines: 9,
		diff: '@@ -667,6 +906,9 @@\n \t\t);\n \t}\n \n+\t/**\n+\t * Returns a query for a commit by ID in a branch outside of a stacking context.\n+\t */\n \tunstackedCommitById(projectId: string, branchName: string, commitId: string, remote?: string) {\n \t\treturn this.api.endpoints.unstackedBranchDetails.useQuery(\n \t\t\t{ projectId, branchName, remote },\n'
	},
	{
		oldStart: 674,
		oldLines: 6,
		newStart: 916,
		newLines: 9,
		diff: '@@ -674,6 +916,9 @@\n \t\t);\n \t}\n \n+\t/**\n+\t * Fetches a page of target commits for a project, starting after the given commit ID.\n+\t */\n \tasync targetCommits(projectId: string, lastCommitId: string | undefined, pageSize: number) {\n \t\treturn await this.api.endpoints.targetCommits.fetch(\n \t\t\t{ projectId, lastCommitId, pageSize },\n'
	},
	{
		oldStart: 685,
		oldLines: 6,
		newStart: 930,
		newLines: 11,
		diff: "@@ -685,6 +930,11 @@\n \t}\n }\n \n+/**\n+ * Injects backend API endpoints for stack, branch, and commit operations.\n+ * @param api The backend API instance\n+ * @returns An object with all endpoints for stack/branch/commit operations\n+ */\n function injectEndpoints(api: ClientState['backendApi']) {\n \treturn api.injectEndpoints({\n \t\tendpoints: (build) => ({\n"
	},
	{
		oldStart: 1343,
		oldLines: 16,
		newStart: 1593,
		newLines: 19,
		diff: '@@ -1343,16 +1593,19 @@\n \t});\n }\n \n+// Entity adapter and selectors for stacks\n const stackAdapter = createEntityAdapter<Stack, string>({\n \tselectId: (stack) => stack.id\n });\n const stackSelectors = { ...stackAdapter.getSelectors(), selectNth: createSelectNth<Stack>() };\n \n+// Entity adapter and selectors for commits\n const commitAdapter = createEntityAdapter<Commit, string>({\n \tselectId: (commit) => commit.id\n });\n const commitSelectors = { ...commitAdapter.getSelectors(), selectNth: createSelectNth<Commit>() };\n \n+// Entity adapter and selectors for upstream commits\n const upstreamCommitAdapter = createEntityAdapter<UpstreamCommit, string>({\n \tselectId: (commit) => commit.id\n });\n'
	},
	{
		oldStart: 1361,
		oldLines: 14,
		newStart: 1614,
		newLines: 17,
		diff: '@@ -1361,14 +1614,17 @@\n \tselectNth: createSelectNth<UpstreamCommit>()\n };\n \n+// Entity adapter and selectors for tree changes (file diffs)\n const changesAdapter = createEntityAdapter<TreeChange, string>({\n \tselectId: (change) => change.path\n });\n \n const changesSelectors = changesAdapter.getSelectors();\n \n+// Selector for changes by file paths\n const selectChangesByPaths = createSelectByIds<TreeChange>();\n \n+// Entity adapter and selectors for branch details\n const branchDetailsAdapter = createEntityAdapter<BranchDetails, string>({\n \tselectId: (branch) => branch.name\n });\n'
	}
];

const MOCK_UNIFIED_DIFF_FILE_A = createMockUnifiedDiffPatch(
	MOCK_FILE_A_MODIFICATION_DIFF_HUNKS,
	20,
	23
);
/**
 * Three branches with file changes.
 */
export default class BranchesWithChanges extends MockBackend {
	dependsOnStack = MOCK_STACK_B_ID;
	bigFileName = MOCK_FILE_J;
	stackWithTwoCommits = MOCK_STACK_B_ID;
	firstCommitInSecondStack = MOCK_COMMIT_IN_BRANCH_B_2;
	prTemplateContent = getMockTemplateContent();

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
		const branchARef = `refs/heads/${MOCK_STACK_A_ID}`;
		stackAChanges.set(branchARef, MOCK_BRANCH_A_CHANGES);

		const stackBChanges = new Map<string, TreeChange[]>();
		const branchBRef = `refs/heads/${MOCK_STACK_B_ID}`;
		stackBChanges.set(branchBRef, MOCK_BRANCH_B_CHANGES);

		const stackCChanges = new Map<string, TreeChange[]>();
		const branchCRef = `refs/heads/${MOCK_STACK_C_ID}`;
		stackCChanges.set(branchCRef, MOCK_BRANCH_C_CHANGES);

		this.branchChanges.set(MOCK_STACK_A_ID, stackAChanges);
		this.branchChanges.set(MOCK_STACK_B_ID, stackBChanges);
		this.branchChanges.set(MOCK_STACK_C_ID, stackCChanges);

		this.unifiedDiffs.set(MOCK_FILE_A, MOCK_UNIFIED_DIFF_FILE_A);
		this.unifiedDiffs.set(MOCK_FILE_D, MOCK_FILE_D_MODIFICATION);
		this.unifiedDiffs.set(MOCK_FILE_J, MOCK_FILE_J_MODIFICATION);

		this.commitChanges.set(MOCK_COMMIT_IN_BRANCH_A.id, MOCK_BRANCH_A_CHANGES);
		this.commitChanges.set(MOCK_COMMIT_IN_BRANCH_B.id, MOCK_COMMIT_B_CHANGES);
		this.commitChanges.set(MOCK_COMMIT_IN_BRANCH_B_2.id, MOCK_COMMIT_B_CHANGES_2);
	}

	getCommitTitle(stackId: string): string {
		if (stackId === MOCK_STACK_A_ID) {
			return MOCK_COMMIT_TITLE_A;
		}
		if (stackId === MOCK_STACK_B_ID) {
			return MOCK_STACK_B_ID;
		}
		if (stackId === MOCK_STACK_C_ID) {
			return MOCK_COMMIT_TITLE_C;
		}

		return '';
	}

	getCommitMessage(stackId: string): string {
		if (stackId === MOCK_STACK_A_ID) {
			return MOCK_COMMIT_MESSAGE_A;
		}
		if (stackId === MOCK_STACK_B_ID) {
			return '';
		}
		if (stackId === MOCK_STACK_C_ID) {
			return MOCK_COMMIT_MESSAGE_C;
		}

		return '';
	}

	commitFailureWithReasons(commitId: string | null): CreateCommitOutcome {
		return {
			newCommit: commitId,
			pathsToRejectedChanges: [
				['cherryPickMergeConflict', MOCK_FILE_1],
				['cherryPickMergeConflict', MOCK_FILE_2],
				['cherryPickMergeConflict', MOCK_FILE_3],
				['workspaceMergeConflict', MOCK_FILE_4],
				['workspaceMergeConflict', MOCK_FILE_5],
				['workspaceMergeConflict', MOCK_FILE_6]
			]
		};
	}

	public getAvailableReviewTemplates(): string[] {
		return ['.github/PULL_REQUEST_TEMPLATE.md'];
	}
}

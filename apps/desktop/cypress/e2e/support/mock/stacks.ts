import type { HunkHeader } from '$lib/hunks/hunk';
import type { Workspace, WorkspaceLegacy } from '@gitbutler/core/api';

export const MOCK_STACK_A_ID = '1234-123';

export const MOCK_STACK_A: WorkspaceLegacy.StackEntry = {
	order: 0,
	id: MOCK_STACK_A_ID,
	heads: [
		{
			name: 'branch-a',
			tip: '1234123',
			isCheckedOut: true
		}
	],
	tip: '1234123',
	isCheckedOut: true
};

export const MOCK_BRAND_NEW_BRANCH_NAME = 'super-cool-branch-name';

export const MOCK_STACK_BRAND_NEW_ID = 'empty-stack';

export const MOCK_STACK_BRAND_NEW: WorkspaceLegacy.StackEntry = {
	order: 1,
	id: MOCK_STACK_BRAND_NEW_ID,
	heads: [
		{
			name: MOCK_BRAND_NEW_BRANCH_NAME,
			tip: '1234123',
			isCheckedOut: false
		}
	],
	tip: '1234123',
	isCheckedOut: false
};

export function createMockStack(
	override: Partial<WorkspaceLegacy.StackEntry>
): WorkspaceLegacy.StackEntry {
	return {
		...MOCK_STACK_A,
		...override
	};
}

export const MOCK_STACKS: WorkspaceLegacy.StackEntry[] = [MOCK_STACK_A];

export const MOCK_AUTHOR: Workspace.Author = {
	name: 'Test Author',
	email: 'author@example.com',
	gravatarUrl: 'https://avatars.githubusercontent.com/u/35891811?v=4'
};

export const MOCK_COMMIT_STATE_LOCAL: Workspace.CommitState = { type: 'LocalOnly' };
export const MOCK_COMMIT_STATE_INTEGRATED: Workspace.CommitState = { type: 'Integrated' };
export const MOCK_COMMIT_STATE_LOCAL_AND_REMOTE_DIVERGED: Workspace.CommitState = {
	type: 'LocalAndRemote',
	subject: 'remote-commit'
};

export const MOCK_COMMIT: Workspace.Commit = {
	id: '1234123',
	parentIds: ['parent-sha'],
	message: 'Initial commit',
	hasConflicts: false,
	state: MOCK_COMMIT_STATE_LOCAL,
	createdAt: BigInt(1714000000000),
	author: MOCK_AUTHOR,
	gerritReviewUrl: null
};

export function createMockCommit(override: Partial<Workspace.Commit>): Workspace.Commit {
	return {
		...MOCK_COMMIT,
		...override
	};
}

export const MOCK_UPSTREAM_COMMIT: Workspace.UpstreamCommit = {
	id: 'upstream-sha',
	message: 'Upstream commit',
	createdAt: BigInt(1714000000001),
	author: MOCK_AUTHOR
};

export function createMockUpstreamCommit(
	override: Partial<Workspace.UpstreamCommit>
): Workspace.UpstreamCommit {
	return {
		...MOCK_UPSTREAM_COMMIT,
		...override
	};
}

export const MOCK_BRANCH_DETAILS: Workspace.BranchDetails = {
	name: 'branch-a',
	linkedWorktreeId: null,
	remoteTrackingBranch: null,
	description: null,
	prNumber: null,
	reviewId: null,
	tip: '1234123',
	baseCommit: 'base-sha',
	pushStatus: 'completelyUnpushed',
	lastUpdatedAt: BigInt(Date.now()),
	authors: [MOCK_AUTHOR],
	isConflicted: false,
	commits: [MOCK_COMMIT],
	upstreamCommits: [],
	isRemoteHead: false
};

export const MOCK_BRANCH_DETAILS_BRAND_NEW: Workspace.BranchDetails = {
	name: MOCK_BRAND_NEW_BRANCH_NAME,
	remoteTrackingBranch: null,
	linkedWorktreeId: null,
	description: null,
	prNumber: null,
	reviewId: null,
	tip: '1234123',
	baseCommit: 'base-sha',
	pushStatus: 'completelyUnpushed',
	lastUpdatedAt: BigInt(Date.now()),
	authors: [],
	isConflicted: false,
	commits: [],
	upstreamCommits: [],
	isRemoteHead: false
};

export function createMockBranchDetails(
	overrides: Partial<Workspace.BranchDetails> = {}
): Workspace.BranchDetails {
	return {
		...MOCK_BRANCH_DETAILS,
		...overrides
	};
}

export const MOCK_STACK_DETAILS_BRAND_NEW: Workspace.StackDetails = {
	derivedName: MOCK_BRAND_NEW_BRANCH_NAME,
	pushStatus: 'completelyUnpushed',
	branchDetails: [MOCK_BRANCH_DETAILS_BRAND_NEW],
	isConflicted: false
};

export const MOCK_STACK_DETAILS: Workspace.StackDetails = {
	derivedName: 'branch-a',
	pushStatus: 'completelyUnpushed',
	branchDetails: [MOCK_BRANCH_DETAILS],
	isConflicted: false
};

export function createMockStackDetails(
	overrides: Partial<Workspace.StackDetails> = {}
): Workspace.StackDetails {
	return {
		...MOCK_STACK_DETAILS,
		...overrides
	};
}

export type UpdateCommitMessageParams = {
	projectId: string;
	stackId: string;
	commitId: string;
	message: string;
};

export function isUpdateCommitMessageParams(params: unknown): params is UpdateCommitMessageParams {
	return (
		typeof params === 'object' &&
		params !== null &&
		'projectId' in params &&
		typeof params.projectId === 'string' &&
		'stackId' in params &&
		typeof params.stackId === 'string' &&
		'commitId' in params &&
		typeof params.commitId === 'string' &&
		'message' in params &&
		typeof params.message === 'string'
	);
}

export type StackDetailsParams = {
	projectId: string;
	stackId: string;
};

export function isStackDetailsParams(params: unknown): params is StackDetailsParams {
	return (
		typeof params === 'object' &&
		params !== null &&
		'projectId' in params &&
		typeof params.projectId === 'string' &&
		'stackId' in params &&
		typeof params.stackId === 'string'
	);
}
export type CreateCommitParamsWorktreeChanges = {
	previousPathBytes?: number[];
	pathBytes: number[];
	hunkHeaders: HunkHeader[];
};

export type CreateCommitParams = {
	stackId: string;
	message: string;
	/** Undefined means that the backend will infer the parent to be the current head of stackBranchName */
	parentId: string | undefined;
	stackBranchName: string;
	worktreeChanges: CreateCommitParamsWorktreeChanges[];
};

export function isHunkHeader(something: unknown): something is HunkHeader {
	return (
		typeof something === 'object' &&
		something !== null &&
		'oldStart' in something &&
		typeof something['oldStart'] === 'number' &&
		'oldLines' in something &&
		typeof something['oldLines'] === 'number' &&
		'newStart' in something &&
		typeof something['newStart'] === 'number' &&
		'newLines' in something &&
		typeof something['newLines'] === 'number'
	);
}

export function isCreateCommitRequestWorktreeChanges(
	something: unknown
): something is CreateCommitParamsWorktreeChanges {
	return (
		typeof something === 'object' &&
		something !== null &&
		((Array.isArray((something as any).previousPathBytes) &&
			(something as any).previousPathBytes.every((byte: any) => typeof byte === 'number')) ||
			(something as any)['previousPathBytes'] === null) &&
		'pathBytes' in something &&
		Array.isArray(something['pathBytes']) &&
		something['pathBytes'].every((byte) => typeof byte === 'number') &&
		'hunkHeaders' in something &&
		Array.isArray(something['hunkHeaders']) &&
		something['hunkHeaders'].every((header) => isHunkHeader(header))
	);
}

export function isCreateCommitParams(args: unknown): args is CreateCommitParams {
	return (
		typeof args === 'object' &&
		args !== null &&
		'stackId' in args &&
		typeof args['stackId'] === 'string' &&
		'message' in args &&
		typeof args['message'] === 'string' &&
		'parentId' in args &&
		(typeof args['parentId'] === 'string' || args['parentId'] === undefined) &&
		'stackBranchName' in args &&
		typeof args['stackBranchName'] === 'string' &&
		'worktreeChanges' in args &&
		Array.isArray(args['worktreeChanges']) &&
		args['worktreeChanges'].every((change) => isCreateCommitRequestWorktreeChanges(change))
	);
}

export type GetTargetCommitsParams = {
	projectId: string;
	lastCommitId: string;
	pageSize: number;
};

export function isGetTargetCommitsParams(params: unknown): params is GetTargetCommitsParams {
	return (
		typeof params === 'object' &&
		params !== null &&
		'projectId' in params &&
		typeof params.projectId === 'string' &&
		'lastCommitId' in params &&
		(params.lastCommitId === undefined || typeof params.lastCommitId === 'string') &&
		'pageSize' in params &&
		typeof params.pageSize === 'number'
	);
}

export type CreateVirtualBranchFromBranchParams = {
	projectId: string;
	branch: string;
	remote?: string;
	prNumber?: number;
};

export function isCreateVirtualBranchFromBranchParams(
	params: unknown
): params is CreateVirtualBranchFromBranchParams {
	return (
		typeof params === 'object' &&
		params !== null &&
		'projectId' in params &&
		typeof params.projectId === 'string' &&
		'branch' in params &&
		typeof params.branch === 'string' &&
		((params as any).remote === undefined || typeof (params as any).remote === 'string') &&
		((params as any).prNumber === undefined || typeof (params as any).prNumber === 'number')
	);
}

export type DeleteLocalBranchParams = {
	projectId: string;
	refname: string;
	givenName: string;
};

export function isDeleteLocalBranchParams(params: unknown): params is DeleteLocalBranchParams {
	return (
		typeof params === 'object' &&
		params !== null &&
		'projectId' in params &&
		typeof params.projectId === 'string' &&
		'refname' in params &&
		typeof params.refname === 'string' &&
		'givenName' in params &&
		typeof params.givenName === 'string'
	);
}
export type SeriesIntegrationStrategy = 'merge' | 'rebase' | 'hardreset';

export type IntegrateUpstreamCommitsParams = {
	projectId: string;
	stackId: string;
	seriesName: string;
	strategy: SeriesIntegrationStrategy | undefined;
};

export function isIntegrateUpstreamCommitsParams(
	params: unknown
): params is IntegrateUpstreamCommitsParams {
	return (
		typeof params === 'object' &&
		params !== null &&
		'projectId' in params &&
		typeof params.projectId === 'string' &&
		'stackId' in params &&
		typeof params.stackId === 'string' &&
		'seriesName' in params &&
		typeof params.seriesName === 'string' &&
		((params as any).strategy === undefined ||
			(params as any).strategy === 'merge' ||
			(params as any).strategy === 'rebase' ||
			(params as any).strategy === 'hardreset')
	);
}

export type PushStackParams = {
	projectId: string;
	stackId: string;
	withForce: boolean;
	branch: string;
};

export function isPushStackParams(params: unknown): params is PushStackParams {
	return (
		typeof params === 'object' &&
		params !== null &&
		'projectId' in params &&
		typeof params.projectId === 'string' &&
		'stackId' in params &&
		typeof params.stackId === 'string' &&
		'withForce' in params &&
		typeof params.withForce === 'boolean' &&
		'branch' in params &&
		typeof params.branch === 'string'
	);
}

export type UpdateBranchPRNumberParams = {
	projectId: string;
	stackId: string;
	branchName: string;
	prNumber: number;
};

export function isUpdateBranchPRNumberParams(
	params: unknown
): params is UpdateBranchPRNumberParams {
	return (
		typeof params === 'object' &&
		params !== null &&
		'projectId' in params &&
		typeof params.projectId === 'string' &&
		'stackId' in params &&
		typeof params.stackId === 'string' &&
		'branchName' in params &&
		typeof params.branchName === 'string' &&
		'prNumber' in params &&
		typeof params.prNumber === 'number'
	);
}

export type UpdateBranchNameParams = {
	projectId: string;
	stackId: string;
	branchName: string;
	newName: string;
};

export function isUpdateBranchNameParams(params: unknown): params is UpdateBranchNameParams {
	return (
		typeof params === 'object' &&
		params !== null &&
		'projectId' in params &&
		typeof params.projectId === 'string' &&
		'stackId' in params &&
		typeof params.stackId === 'string' &&
		'branchName' in params &&
		typeof params.branchName === 'string' &&
		'newName' in params &&
		typeof params.newName === 'string'
	);
}

export type RemoveBranchParams = {
	projectId: string;
	stackId: string;
	branchName: string;
};

export function isRemoveBranchParams(params: unknown): params is RemoveBranchParams {
	return (
		typeof params === 'object' &&
		params !== null &&
		'projectId' in params &&
		typeof params.projectId === 'string' &&
		'stackId' in params &&
		typeof params.stackId === 'string' &&
		'branchName' in params &&
		typeof params.branchName === 'string'
	);
}

export type CreateBranchParams = {
	projectId: string;
	stackId: string;
	request: { targetPatch?: string; name: string };
};

export function isCreateBranchParams(params: unknown): params is CreateBranchParams {
	return (
		typeof params === 'object' &&
		params !== null &&
		'projectId' in params &&
		typeof params.projectId === 'string' &&
		'stackId' in params &&
		typeof params.stackId === 'string' &&
		'request' in params &&
		typeof (params as any).request === 'object' &&
		params.request !== null &&
		((params.request as any).targetPatch === undefined ||
			typeof (params.request as any).targetPatch === 'string') &&
		typeof (params.request as any).name === 'string'
	);
}

type BranchParams = {
	name?: string;
	ownership?: string;
	order?: number;
	allow_rebasing?: boolean;
	notes?: string;
	selected_for_changes?: boolean;
};

function isBranchParams(params: unknown): params is BranchParams {
	return (
		typeof params === 'object' &&
		params !== null &&
		((params as BranchParams).name === undefined ||
			typeof (params as BranchParams).name === 'string') &&
		((params as BranchParams).ownership === undefined ||
			typeof (params as BranchParams).ownership === 'string') &&
		((params as BranchParams).order === undefined ||
			typeof (params as BranchParams).order === 'number') &&
		((params as BranchParams).allow_rebasing === undefined ||
			typeof (params as BranchParams).allow_rebasing === 'boolean') &&
		((params as BranchParams).notes === undefined ||
			typeof (params as BranchParams).notes === 'string') &&
		((params as BranchParams).selected_for_changes === undefined ||
			typeof (params as BranchParams).selected_for_changes === 'boolean')
	);
}
export type CreateStackParams = {
	projectId: string;
	branch: BranchParams;
};

export function isCreateStackParams(params: unknown): params is CreateStackParams {
	return (
		typeof params === 'object' &&
		params !== null &&
		'projectId' in params &&
		typeof (params as CreateStackParams).projectId === 'string' &&
		'branch' in params &&
		isBranchParams((params as CreateStackParams).branch)
	);
}

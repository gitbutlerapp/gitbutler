const MOCK_REMOTE_BRANCH_A = 'refs/remotes/origin/feature-branch-a';
const MOCK_REMOTE_BRANCH_B = 'refs/remotes/origin/feature-branch-b';

export function getRemoteBranches(): string[] {
	return [MOCK_REMOTE_BRANCH_A, MOCK_REMOTE_BRANCH_B];
}

const MOCK_AUTHOR = {
	email: 'author@committer.com',
	name: 'Author Name'
};

const MOCK_COMMIT = {
	id: 'abc123',
	author: MOCK_AUTHOR,
	description: 'Initial commit',
	createdAt: new Date('2023-01-01T00:00:00Z').getTime(),
	changeId: '1',
	isSigned: false,
	parentIds: [],
	conflicted: false,
	prev: undefined,
	next: undefined
};

const MOCK_RECENT_COMMITS = [MOCK_COMMIT];

const MOCK_BASE_BRANCH_DATA = {
	branchName: 'origin/main',
	remoteName: 'origin',
	remoteUrl: 'https://github.com/example/repo.git',
	pushRemoteName: 'origin',
	pushRemoteUrl: null,
	baseSha: 'abc123',
	currentSha: 'abc123',
	behind: 0,
	upstreamCommits: [],
	recentCommits: MOCK_RECENT_COMMITS,
	lastFetchedMs: Date.now(),
	conflicted: false,
	diverged: false,
	divergedAhead: [],
	divergedBehind: [],
	forgeRepoInfo: {
		forge: 'github',
		owner: 'example',
		repo: 'repo',
		protocol: 'https'
	}
};

export type BaseBranchData = typeof MOCK_BASE_BRANCH_DATA;

export function mockBaseBranchData(override: Partial<BaseBranchData> = {}): BaseBranchData {
	return {
		...MOCK_BASE_BRANCH_DATA,
		...override
	};
}

export function getBaseBranchData() {
	return MOCK_BASE_BRANCH_DATA;
}

export function getBaseBranchBehindData() {
	return {
		...MOCK_BASE_BRANCH_DATA,
		behind: 1,
		upstreamCommits: [
			{
				...MOCK_COMMIT,
				id: 'upstream-commit-id',
				description: 'Upstream commit'
			}
		]
	};
}

export type GetBaseBranchParams = {
	projectId: string;
};

export function isGetBaseBranchArgs(args: unknown): args is GetBaseBranchParams {
	return (
		typeof args === 'object' &&
		args !== null &&
		'projectId' in args &&
		typeof (args as GetBaseBranchParams).projectId === 'string'
	);
}

export type SetBaseBranchParams = {
	projectId: string;
	branch: string;
	pushRemote?: string;
	stashUncommitted?: boolean;
};

export function isSetBaseBranchArgs(args: unknown): args is SetBaseBranchParams {
	return (
		typeof args === 'object' &&
		args !== null &&
		'projectId' in args &&
		typeof (args as SetBaseBranchParams).projectId === 'string' &&
		'branch' in args &&
		typeof (args as SetBaseBranchParams).branch === 'string' &&
		(!('pushRemote' in args) || typeof (args as SetBaseBranchParams).pushRemote === 'string') &&
		(!('stashUncommitted' in args) ||
			typeof (args as SetBaseBranchParams).stashUncommitted === 'boolean')
	);
}

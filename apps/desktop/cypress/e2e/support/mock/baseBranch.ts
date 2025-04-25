const MOCK_REMOTE_BRANCH_A = 'refs/remotes/origin/feature-branch-a';
const MOCK_REMOTE_BRANCH_B = 'refs/remotes/origin/feature-branch-b';

export function getRemoteBranches(): string[] {
	return [MOCK_REMOTE_BRANCH_A, MOCK_REMOTE_BRANCH_B];
}

const MOCK_AUTHOR = {
	email: 'author@committer.com',
	name: 'Author Name'
};

const MOCK_RECENT_COMMITS = [
	{
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
	}
];

const MOCK_BASE_BRANCH_DATA = {
	branchName: 'origin/main',
	remoteName: 'origin',
	remoteUrl: 'https://example.com/repo.git',
	pushRemoteName: 'origin',
	pushRemoteUrl: 'https://example.com/repo.git',
	baseSha: 'abc123',
	currentSha: 'abc123',
	behind: 0,
	upstreamCommits: [],
	recentCommits: MOCK_RECENT_COMMITS,
	lastFetchedMs: Date.now(),
	conflicted: false,
	diverged: false,
	divergedAhead: [],
	divergedBehind: []
};

export function getBaseBranchData() {
	return MOCK_BASE_BRANCH_DATA;
}

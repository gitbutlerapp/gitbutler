import { vi } from 'vitest';

export function getMockBaseBranchCommit() {
	const MockBaseBranchCommit = vi.fn();

	return MockBaseBranchCommit;
}

export function getMockCommit() {
	const MockCommit = vi.fn();

	MockCommit.prototype.id = 'mock-id';
	MockCommit.prototype.author = {
		name: 'Mock Author',
		email: 'mock@example.com',
		gravatarUrl: '',
		isBot: false
	};
	MockCommit.prototype.description = 'Mock description';
	MockCommit.prototype.createdAt = new Date();
	MockCommit.prototype.changeId = 'mock-change-id';
	MockCommit.prototype.isSigned = false;
	MockCommit.prototype.parentIds = [];
	MockCommit.prototype.conflicted = false;
	MockCommit.prototype.prev = undefined;
	MockCommit.prototype.next = undefined;
	MockCommit.prototype.relatedTo = undefined;
	MockCommit.prototype.descriptionTitle = vi.fn(() => 'Mock Title');
	MockCommit.prototype.descriptionBody = vi.fn(() => 'Mock Body');
	MockCommit.prototype.status = vi.fn(() => 'Remote');
	MockCommit.prototype.isMergeCommit = vi.fn(() => false);
	MockCommit.prototype.conflictedFiles = vi.fn(() => ({
		ancestorEntries: [],
		ourEntries: [],
		theirEntries: []
	}));

	return MockCommit;
}

export function getMockBaseBranch() {
	const MockBaseBranch = vi.fn();

	MockBaseBranch.prototype.branchName = 'mock-branch';
	MockBaseBranch.prototype.remoteName = 'mock-remote';
	MockBaseBranch.prototype.remoteUrl = 'https://mock.remote.url';
	MockBaseBranch.prototype.pushRemoteName = 'mock-push-remote';
	MockBaseBranch.prototype.pushRemoteUrl = 'https://mock.push.remote.url';
	MockBaseBranch.prototype.baseSha = 'mock-base-sha';
	MockBaseBranch.prototype.currentSha = 'mock-current-sha';
	MockBaseBranch.prototype.behind = 0;
	MockBaseBranch.prototype.upstreamCommits = [];
	MockBaseBranch.prototype.recentCommits = [];
	MockBaseBranch.prototype.lastFetchedMs = undefined;
	MockBaseBranch.prototype.conflicted = false;
	MockBaseBranch.prototype.diverged = false;
	MockBaseBranch.prototype.divergedAhead = [];
	MockBaseBranch.prototype.divergedBehind = [];
	MockBaseBranch.prototype.actualPushRemoteName = vi.fn(() => 'mock-remote');
	MockBaseBranch.prototype.lastFetched = vi.fn(() => undefined);
	MockBaseBranch.prototype.shortName = vi.fn(() => 'mock-branch');

	return MockBaseBranch;
}

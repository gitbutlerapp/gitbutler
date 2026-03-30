import type { BaseBranch } from "$lib/baseBranch/baseBranch";
import type { Commit } from "$lib/commits/commit";

export function getMockCommit(): Commit {
	return {
		id: "mock-id",
		author: {
			name: "Mock Author",
			email: "mock@example.com",
			gravatarUrl: "",
			isBot: false,
		},
		description: "Mock description",
		createdAt: Date.now(),
		changeId: "mock-change-id",
		isSigned: false,
		parentIds: [],
		conflicted: false,
	};
}

export function getMockBaseBranch(): BaseBranch {
	return {
		branchName: "mock-branch",
		remoteName: "mock-remote",
		remoteUrl: "https://mock.remote.url",
		pushRemoteName: "mock-push-remote",
		pushRemoteUrl: "https://mock.push.remote.url",
		baseSha: "mock-base-sha",
		currentSha: "mock-current-sha",
		behind: 0,
		upstreamCommits: [],
		recentCommits: [],
		lastFetchedMs: undefined,
		conflicted: false,
		diverged: false,
		divergedAhead: [],
		divergedBehind: [],
		shortName: "mock-branch",
	};
}

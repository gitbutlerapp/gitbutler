import type { BaseBranch } from "@gitbutler/but-sdk";

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
		lastFetchedMs: null,
		conflicted: false,
		diverged: false,
		divergedAhead: [],
		divergedBehind: [],
		shortName: "mock-branch",
	};
}

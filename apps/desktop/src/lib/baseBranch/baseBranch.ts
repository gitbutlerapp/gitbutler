import type { Commit } from "$lib/commits/commit";

export interface RemoteBranchInfo {
	name: string;
}

export type ForgeProvider = "github" | "gitlab" | "bitbucket" | "azure";

export type ForgeRepoInfo = {
	forge: ForgeProvider;
	owner: string;
	repo: string;
	protocol: string;
};

export interface BaseBranch {
	branchName: string;
	remoteName: string;
	remoteUrl: string;
	/** Resolved push remote name — falls back to remoteName if unset. */
	pushRemoteName: string;
	pushRemoteUrl: string;
	baseSha: string;
	currentSha: string;
	behind: number;
	upstreamCommits: Commit[];
	recentCommits: Commit[];
	lastFetchedMs?: number;
	conflicted: boolean;
	diverged: boolean;
	divergedAhead: string[];
	divergedBehind: string[];
	/** Branch name with remote and ref prefixes stripped. */
	shortName: string;
}

export function lastFetched(branch: BaseBranch): Date | undefined {
	return branch.lastFetchedMs ? new Date(branch.lastFetchedMs) : undefined;
}

import type { BaseBranch } from "@gitbutler/but-sdk";

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

export function lastFetched(branch: BaseBranch): Date | undefined {
	return branch.lastFetchedMs ? new Date(branch.lastFetchedMs) : undefined;
}

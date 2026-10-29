import { parseRemoteUrl } from "$lib/git/gitUrl";
import type { PullRequest } from "$lib/forge/interface/types";

/**
 * Whether `pr` targets the project's base branch. Without the
 * base-repo check, a fork PR using the same branch name as the user's
 * base would enable "Merge" against a stranger's fork. GitLab and
 * Bitbucket are branch-only: GitLab's fork model uses numeric project
 * IDs rather than URL identity.
 */
export function pullRequestTargetsBaseBranch(args: {
	pr: PullRequest | undefined;
	baseBranchShortName: string | undefined;
	baseBranchRepoHash: string | undefined;
	prBaseRepoUrl: string | null | undefined;
	forgeName: string | undefined;
}): boolean {
	const { pr, baseBranchShortName, baseBranchRepoHash, prBaseRepoUrl, forgeName } = args;
	if (!pr) return false;
	if (pr.targetBranch !== baseBranchShortName) return false;
	if (forgeName === "gitlab" || forgeName === "bitbucket") return true;
	if (!prBaseRepoUrl || !baseBranchRepoHash) return false;
	const prBaseRepo = parseRemoteUrl(prBaseRepoUrl);
	return prBaseRepo?.hash === baseBranchRepoHash;
}

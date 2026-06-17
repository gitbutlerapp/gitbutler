import type { RepoInfo } from "$lib/git/gitUrl";
import type { ForgeReviewLabel } from "@gitbutler/but-sdk";
import type { Reactive } from "@gitbutler/shared/storeUtils";

/** Author as represented in forge contexts (GitHub/GitLab PR). Fields are optional since forges may omit them. */
export type Author = {
	login?: string;
	name?: string;
	email?: string;
	gravatarUrl?: string;
	isBot?: boolean;
};

export type ForgeUser = {
	login: string;
	srcUrl: string;
	name: string;
};

/**
 * A display-ready forge user resolved reactively from the project's
 * preferred account. Stable across re-renders so the producing hook can
 * be called once at component init (its `inject()` must not run inside a
 * reactive re-computation).
 */
export type ForgeUserQuery = {
	user: Reactive<ForgeUser | undefined>;
	isLoading: Reactive<boolean>;
};

export type ForgeUserDetailed = {
	id: number;
	login: string;
	name: string | null;
	email: string | null;
	avatarUrl: string | null;
	isBot: boolean;
};

export type ForgeReview = {
	htmlUrl: string;
	number: number;
	title: string;
	body: string | null;
	author: ForgeUserDetailed | null;
	labels: ForgeReviewLabel[];
	draft: boolean;
	sourceBranch: string;
	targetBranch: string;
	sha: string;
	createdAt: string | null;
	modifiedAt: string | null;
	mergedAt: string | null;
	closedAt: string | null;
	repositorySshUrl: string | null;
	repositoryHttpsUrl: string | null;
	repoOwner: string | null;
	reviewers: ForgeUserDetailed[];
};

export interface PullRequest {
	htmlUrl: string;
	number: number;
	title: string;
	body: string | undefined;
	author: Author | null;
	labels: ForgeReviewLabel[];
	draft: boolean;
	sourceBranch: string;
	targetBranch: string;
	sha: string;
	createdAt: string;
	modifiedAt: string;
	mergedAt?: string;
	closedAt?: string;
	repositorySshUrl?: string;
	repositoryHttpsUrl?: string;
	repoOwner?: string;
	reviewers: ForgeUser[];
}

export function mapForgeReviewToPullRequest(pr: ForgeReview): PullRequest {
	return {
		htmlUrl: pr.htmlUrl,
		number: pr.number,
		title: pr.title,
		body: pr.body ?? undefined,
		author: pr.author
			? {
					login: pr.author.login,
					name: pr.author.name ?? pr.author.login,
					email: pr.author.email ?? undefined,
					gravatarUrl: pr.author.avatarUrl ?? undefined,
					isBot: pr.author.isBot,
				}
			: null,
		labels: pr.labels,
		draft: pr.draft,
		sourceBranch: pr.sourceBranch,
		targetBranch: pr.targetBranch,
		sha: pr.sha,
		createdAt: pr.createdAt ?? "",
		modifiedAt: pr.modifiedAt ?? "",
		mergedAt: pr.mergedAt ?? undefined,
		closedAt: pr.closedAt ?? undefined,
		repositorySshUrl: pr.repositorySshUrl ?? undefined,
		repositoryHttpsUrl: pr.repositoryHttpsUrl ?? undefined,
		repoOwner: pr.repoOwner ?? undefined,
		reviewers: pr.reviewers.map((r) => ({
			login: r.login,
			srcUrl: r.avatarUrl ?? "",
			name: r.name ?? r.login,
		})),
	};
}

export type ChecksStatus = {
	/**
	 * Earliest check-run start, or `null` while every run is still
	 * queued. Must not fall back to "now" — `CIChecksBadge` keys
	 * polling backoff off this value.
	 */
	startedAt: string | null;
	/**
	 * Checks are considered completed if all checks have completed  or if there is at least one failure.
	 */
	completed: boolean;
	success: boolean;
	failedChecks: string[];
	actionRequired: boolean;
	actionRequiredChecks: string[];
};

export enum MergeMethod {
	Merge = "merge",
	Rebase = "rebase",
	Squash = "squash",
}

export type CheckSuite = {
	name?: string;
	count: number;
	status: "queued" | "in_progress" | "completed" | "waiting" | "requested" | "pending" | null;
};

export type ForgeArguments = {
	repo: RepoInfo;
	baseBranch: string;
	forkStr?: string;
	authenticated: boolean;
};

export type CreatePullRequestArgs = {
	title: string;
	body: string;
	draft: boolean;
	baseBranchName: string;
	upstreamName: string;
};

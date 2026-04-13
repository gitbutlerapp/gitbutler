import type {
	DetailedPullRequest,
	Label,
	MergeMethod,
	PullRequest,
} from "$lib/forge/interface/types";
import type { ButGitea } from "@gitbutler/core/api";

export function giteaResponseToPullRequest(pr: ButGitea.PullRequest): PullRequest {
	return {
		htmlUrl: pr.htmlUrl,
		number: pr.number,
		title: pr.title,
		body: pr.body ?? undefined,
		author: pr.author
			? {
					name: pr.author.fullName ?? pr.author.login,
					email: pr.author.email ?? undefined,
					gravatarUrl: pr.author.avatarUrl ?? undefined,
				}
			: null,
		labels: pr.labels.map((l) => ({
			name: l.name,
			description: l.description ?? undefined,
			color: l.color ?? "000000",
		})),
		draft: pr.draft,
		sourceBranch: pr.sourceBranch,
		targetBranch: pr.targetBranch,
		sha: pr.sha,
		createdAt: pr.createdAt ?? "",
		modifiedAt: pr.modifiedAt ?? "",
		mergedAt: pr.mergedAt ?? undefined,
		closedAt: pr.closedAt ?? undefined,
		reviewers: [], // Gitea API might not return these in the simple PR object
	};
}

export function giteaResponseToDetailedPullRequest(pr: ButGitea.PullRequest): DetailedPullRequest {
	return {
		id: pr.number,
		title: pr.title,
		author: pr.author
			? {
					name: pr.author.fullName ?? pr.author.login,
					email: pr.author.email ?? undefined,
					gravatarUrl: pr.author.avatarUrl ?? undefined,
				}
			: null,
		body: pr.body ?? undefined,
		number: pr.number,
		sourceBranch: pr.sourceBranch,
		draft: pr.draft,
		fork: false, // Need to check if it's a fork
		createdAt: pr.createdAt ?? "",
		mergedAt: pr.mergedAt ?? undefined,
		closedAt: pr.closedAt ?? undefined,
		updatedAt: pr.modifiedAt ?? "",
		htmlUrl: pr.htmlUrl,
		merged: !!pr.mergedAt,
		mergeable: true, // Need to verify from API
		mergeableState: "unknown",
		rebaseable: true,
		squashable: true,
		state: pr.closedAt ? "closed" : "open",
		baseBranch: pr.targetBranch,
		reviewers: [],
		commentsCount: 0,
	};
}

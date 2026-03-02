import type { DetailedPullRequest, Label, PullRequest } from "$lib/forge/interface/types";
import type {
	PullRequest as GiteaPullRequest,
	User as GiteaUser,
	Repository as GiteaRepository,
	Permission as GiteaPermission,
} from "gitea-js";

type Data<T> = { data: T };

export function detailedPrToInstance(
	response: Data<GiteaPullRequest>,
	permissions: GiteaPermission | undefined = undefined,
): DetailedPullRequest {
	const data = response.data;

	const reviewers =
		data.requested_reviewers?.map((reviewer) => ({
			srcUrl: reviewer.avatar_url || "",
			username: reviewer.full_name || reviewer.login || "",
		})) || [];

	return {
		id: data.id || -1,
		number: data.number || -1,
		author: data.user
			? {
					name: data.user.full_name || data.user.login || undefined,
					email: data.user.email || undefined,
					isBot: !!data.user.is_admin === false,
					gravatarUrl: data.user.avatar_url,
				}
			: null,
		title: data.title || `${data.number}` || "",
		body: data.body ?? undefined,
		baseBranch: data.base?.ref || "",
		sourceBranch: data.head?.ref || "",
		draft: !!data.draft,
		htmlUrl: data.html_url || "",
		createdAt: data.created_at || "",
		mergedAt: data.merged_at || undefined,
		closedAt: data.closed_at || undefined,
		updatedAt: data.updated_at || data.created_at || "",
		merged: !!data.merged,
		mergeable: !!data.mergeable,
		mergeableState: data.state ?? "unknown",
		rebaseable: !!data.base?.repo?.allow_rebase,
		squashable: !!data.base?.repo?.allow_squash_merge,
		state: data.state === "open" ? "open" : "closed",
		fork: !!data.head?.repo?.fork,
		reviewers,
		commentsCount: data.comments || 0,
		permissions: {
			canMerge: !!permissions?.push,
		},
	};
}

export function prToInstance(response: Data<GiteaPullRequest>): PullRequest {
	const data = response.data;

	const labels: Label[] =
		data.labels?.map((label) => ({
			name: label.name || "",
			description: label.description || undefined,
			color: label.color || "pink",
		})) || [];

	const reviewers =
		data.requested_reviewers?.map((reviewer) => ({
			id: reviewer.id || 0,
			srcUrl: reviewer.avatar_url || "",
			name: reviewer.full_name || reviewer.login || "",
		})) || [];

	return {
		number: data.number || -1,
		author: data.user
			? {
					name: data.user.full_name || data.user.login || undefined,
					email: data.user.email || undefined,
					isBot: false,
					gravatarUrl: data.user.avatar_url,
				}
			: null,
		title: data.title || `${data.number}` || "",
		body: data.body ?? undefined,
		sourceBranch: data.head?.ref || "",
		targetBranch: data.base?.ref || "",
		htmlUrl: data.html_url || "",
		createdAt: data.created_at || "",
		modifiedAt: data.updated_at || data.created_at || "",
		mergedAt: data.merged_at || undefined,
		closedAt: data.closed_at || undefined,
		draft: !!data.draft,
		labels,
		reviewers,
		sha: data.head?.sha || "",
	};
}

export function userToInstance(response: Data<GiteaUser>) {
	const user = response.data;
	return {
		id: user.id || 0,
		name: user.full_name || user.login || "",
		srcUrl: user.avatar_url || "",
	};
}

export function repoToInstance(response: Data<GiteaRepository>) {
	const repo = response.data;
	return {
		id: repo.id || -1,
		name: repo.name || "",
		fullName: `${repo.owner?.login}/${repo.name}` || "",
		htmlUrl: repo.html_url || "",
		httpsUrl: repo.clone_url || "",
		sshUrl: repo.ssh_url || "",
		permissions: repo.permissions,
	};
}

export type GiteaProjectId = `${string}/${string}`;

export function splitGiteaProjectId(projectId: GiteaProjectId): {
	owner: string;
	repo: string;
} {
	const parts = projectId.split("/");

	const owner = parts.at(0);
	const repo = parts.at(1);

	if (!owner || !repo) {
		throw new Error(`Invalid Gitea project ID: ${projectId}`);
	}
	return { owner, repo };
}

export function isValidGiteaProjectId(projectId?: string): projectId is GiteaProjectId {
	const parts = projectId?.split("/") ?? [];
	return parts.length === 2 && parts.every((part) => part.length > 0);
}

import type { DetailedPullRequest, Label, PullRequest } from '$lib/forge/interface/types';
import type {
	PullRequest as GiteaPullRequest,
	User as GiteaUser,
	Repository as GiteaRepository,
	Permission as GiteaPermission
} from 'gitea-js';

type Data<T> = { data: T };

export function detailedPrToInstance(
	response: Data<GiteaPullRequest>,
	permissions: GiteaPermission | undefined = undefined
): DetailedPullRequest {
	const data = response.data;

	const reviewers =
		data.requested_reviewers?.map((assignee) => ({
			srcUrl: assignee.avatar_url || '',
			name: assignee?.full_name || assignee?.login || ''
		})) || [];

	return {
		id: data.id || -1,
		number: data?.number || -1,
		author: data.user ? {} : null,
		title: data.title || `${data.number}` || '',
		body: data.body ?? undefined,
		baseBranch: data.base?.ref || '',
		sourceBranch: data.head?.ref || '',
		draft: data.draft,
		htmlUrl: data.html_url || '',
		createdAt: data.created_at || '',
		mergedAt: data.merged_at || undefined,
		closedAt: data.closed_at || undefined,
		updatedAt: data.updated_at || data.created_at || '',
		merged: !!data.merged,
		mergeable: !!data.mergeable,
		mergeableState: data.state ?? 'unknown',
		rebaseable: !!data.base?.repo?.allow_rebase,
		squashable: !!data.base?.repo?.allow_squash_merge,
		state: data.state === 'opened' ? 'open' : 'closed',
		fork: !!data.head?.repo?.fork,
		reviewers,
		commentsCount: data?.comments || 0,
		permissions: {
			canMerge: !!permissions?.push
		}
	};
}

export function prToInstance(response: Data<GiteaPullRequest>): PullRequest {
	const data = response.data;

	const reviewers =
		data.requested_reviewers?.map((assignee) => ({
			srcUrl: assignee.avatar_url || '',
			name: assignee?.full_name || assignee?.login || ''
		})) || [];

	return {
		number: data?.number || -1,
		author: data.user ? {} : null,
		title: data.title || `${data.number}` || '',
		body: data.body ?? undefined,
		sourceBranch: data.head?.ref || '',
		htmlUrl: data.html_url || '',
		createdAt: data.created_at || '',
		mergedAt: data.merged_at || undefined,
		closedAt: data.closed_at || undefined,
		modifiedAt: data.updated_at || data.created_at || '',
		draft: !!data.draft,
		reviewers,
		labels: data?.labels?.map((e) => ({ name: '', description: '', color: '', ...e })) || [],
		targetBranch: data.base?.ref || '',
		sha: data.head?.sha || ''
	};
}

export function userToInstance(response: Data<GiteaUser>) {
	const user = response.data;
	return {
		name: user.full_name || undefined,
		email: user.email || undefined,
		isBot: false,
		gravatarUrl: user.avatar_url
	};
}

export function repoToInstance(response: Data<GiteaRepository>) {
	const repo = response.data;
	return {
		id: repo.id || -1,
		name: repo.name || '',
		fullName: `${repo.owner?.login}/${repo.name}` || '',
		htmlUrl: repo.html_url || '',
		httpsUrl: repo.clone_url || '',
		sshUrl: repo.ssh_url || '',
		permissions: repo.permissions
	};
}

export type GiteaProjectId = `${string}/${string}`;

export function splitGiteaProjectId(projectId: GiteaProjectId): {
	owner: string;
	repo: string;
} {
	const parts = projectId.split('/');

	const owner = parts.at(0);
	const repo = parts.at(1);

	if (!owner || !repo) {
		throw new Error(`Invalid Gitea project ID: ${projectId}`);
	}
	return { owner, repo };
}

export function isValidGiteaProjectId(projectId?: string): projectId is GiteaProjectId {
	const parts = projectId?.split('/') ?? [];
	return parts.length === 2 && parts.every((part) => part.length > 0);
}

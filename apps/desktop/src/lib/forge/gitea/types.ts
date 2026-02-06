import type { DetailedPullRequest, Label, PullRequest } from '$lib/forge/interface/types';

// Gitea API response types

export interface GiteaApiUser {
	id: number;
	login: string;
	full_name: string;
	email: string;
	avatar_url: string;
}

export interface GiteaApiLabel {
	id: number;
	name: string;
	description: string;
	color: string;
}

export interface GiteaApiRepo {
	id: number;
	full_name: string;
	ssh_url: string;
	clone_url: string;
	owner: GiteaApiUser;
}

export interface GiteaApiPullRequest {
	id: number;
	number: number;
	title: string;
	body: string;
	state: 'open' | 'closed';
	draft: boolean;
	html_url: string;
	created_at: string;
	updated_at: string;
	merged_at: string | null;
	closed_at: string | null;
	merge_commit_sha: string | null;
	mergeable: boolean;
	merged: boolean;
	user: GiteaApiUser;
	labels: GiteaApiLabel[];
	head: {
		label: string;
		ref: string;
		sha: string;
		repo: GiteaApiRepo;
	};
	base: {
		label: string;
		ref: string;
		sha: string;
		repo: GiteaApiRepo;
	};
	requested_reviewers: GiteaApiUser[];
	comments: number;
}

export function giteaPrToInstance(pr: GiteaApiPullRequest): PullRequest {
	const labels: Label[] =
		pr.labels?.map((label) => ({
			name: label.name,
			description: label.description || undefined,
			color: `#${label.color}`
		})) ?? [];

	const reviewers =
		pr.requested_reviewers?.map((reviewer) => ({
			id: reviewer.id,
			srcUrl: reviewer.avatar_url,
			name: reviewer.full_name || reviewer.login
		})) ?? [];

	return {
		htmlUrl: pr.html_url,
		number: pr.number,
		title: pr.title,
		body: pr.body || undefined,
		author: pr.user
			? {
					id: pr.user.id,
					name: pr.user.full_name || pr.user.login || undefined,
					email: pr.user.email || undefined,
					isBot: false,
					gravatarUrl: pr.user.avatar_url
				}
			: null,
		labels,
		draft: pr.draft || false,
		createdAt: pr.created_at,
		modifiedAt: pr.updated_at,
		sourceBranch: pr.head.ref,
		targetBranch: pr.base.ref,
		sha: pr.head.sha,
		mergedAt: pr.merged_at || undefined,
		closedAt: pr.closed_at || undefined,
		repoOwner: pr.head.repo?.owner?.login ?? '',
		repositorySshUrl: pr.head.repo?.ssh_url ?? '',
		repositoryHttpsUrl: pr.head.repo?.clone_url ?? '',
		reviewers
	};
}

export function giteaPrToDetailedInstance(pr: GiteaApiPullRequest): DetailedPullRequest {
	const reviewers =
		pr.requested_reviewers?.map((reviewer) => ({
			srcUrl: reviewer.avatar_url,
			username: reviewer.full_name || reviewer.login
		})) ?? [];

	return {
		id: pr.id,
		number: pr.number,
		author: pr.user
			? {
					name: pr.user.full_name || pr.user.login || undefined,
					email: pr.user.email || undefined,
					isBot: false,
					gravatarUrl: pr.user.avatar_url
				}
			: null,
		title: pr.title,
		body: pr.body ?? undefined,
		baseBranch: pr.base.ref,
		sourceBranch: pr.head.ref,
		draft: pr.draft,
		htmlUrl: pr.html_url,
		createdAt: pr.created_at,
		mergedAt: pr.merged_at || undefined,
		closedAt: pr.closed_at || undefined,
		updatedAt: pr.updated_at,
		merged: pr.merged,
		mergeable: pr.mergeable,
		mergeableState: pr.mergeable ? 'mergeable' : 'not_mergeable',
		rebaseable: pr.mergeable,
		squashable: pr.mergeable,
		state: pr.state === 'open' ? 'open' : 'closed',
		fork: pr.head.repo?.full_name !== pr.base.repo?.full_name,
		reviewers,
		commentsCount: pr.comments ?? 0,
		permissions: {
			canMerge: pr.mergeable
		},
		repositorySshUrl: pr.head.repo?.ssh_url,
		repositoryHttpsUrl: pr.head.repo?.clone_url
	};
}

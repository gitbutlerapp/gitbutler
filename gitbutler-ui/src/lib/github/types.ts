import type { Author } from '$lib/vbranches/types';
import type { RestEndpointMethodTypes } from '@octokit/rest';

export interface GitHubIntegrationContext {
	authToken: string;
	owner: string;
	repo: string;
	username: string;
}

export interface Label {
	name: string;
	description: string | undefined;
	color: string;
}

export interface PullRequest {
	htmlUrl: string;
	number: number;
	title: string;
	body: string | undefined;
	author: Author | null;
	labels: Label[];
	draft: boolean;
	targetBranch: string;
	sourceBranch: string;
	createdAt: Date;
	modifiedAt: Date;
	mergedAt?: Date;
	closedAt?: Date;
}

export type ChecksStatus =
	| {
			startedAt?: Date;
			completed?: boolean;
			success?: boolean;
			hasChecks?: boolean;
			error?: any;
	  }
	| null
	| undefined;

export function ghResponseToInstance(
	pr:
		| RestEndpointMethodTypes['pulls']['create']['response']['data']
		| RestEndpointMethodTypes['pulls']['list']['response']['data'][number]
): PullRequest {
	const labels: Label[] = pr.labels.map((label) => {
		return {
			name: label.name,
			description: label.description || undefined,
			color: label.color
		};
	});

	return {
		htmlUrl: pr.html_url,
		number: pr.number,
		title: pr.title,
		body: pr.body || undefined,
		author: pr.user
			? {
					name: pr.user.login || undefined,
					email: pr.user.email || undefined,
					isBot: pr.user.type.toLowerCase() == 'bot',
					gravatarUrl: new URL(pr.user.avatar_url)
				}
			: null,
		labels: labels,
		draft: pr.draft || false,
		createdAt: new Date(pr.created_at),
		modifiedAt: new Date(pr.created_at),
		targetBranch: pr.head.ref,
		sourceBranch: pr.base.ref,
		mergedAt: pr.merged_at ? new Date(pr.merged_at) : undefined,
		closedAt: pr.closed_at ? new Date(pr.closed_at) : undefined
	};
}

export enum MergeMethod {
	Merge = 'merge',
	Rebase = 'rebase',
	Squash = 'squash'
}

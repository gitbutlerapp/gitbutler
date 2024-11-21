import { parseRemoteUrl } from '$lib/url/gitUrl';
import type {
	ChecksStatus,
	CheckSuite,
	DetailedPullRequest,
	Label,
	PullRequest
} from '../interface/types';
import type { RestEndpointMethodTypes } from '@octokit/rest';

export type DetailedGitHubPullRequest = RestEndpointMethodTypes['pulls']['get']['response']['data'];

export function parseGitHubDetailedPullRequest(
	data: DetailedGitHubPullRequest
): DetailedPullRequest {
	return {
		id: data.id,
		number: data.number,
		title: data.title,
		body: data.body ?? undefined,
		baseRepo: parseRemoteUrl(data.base?.repo.git_url),
		baseBranch: data.base?.ref,
		sourceBranch: data.head?.ref,
		draft: data.draft,
		htmlUrl: data.html_url,
		createdAt: new Date(data.created_at),
		mergedAt: data.merged_at ? new Date(data.merged_at) : undefined,
		closedAt: data.closed_at ? new Date(data.closed_at) : undefined,
		merged: data.merged,
		mergeable: !!data.mergeable,
		mergeableState: data.mergeable_state,
		rebaseable: !!data.rebaseable,
		squashable: !!data.mergeable, // Enabled whenever merge is enabled
		state: data.state,
		fork: data.head?.repo?.fork ?? false
	};
}

export function ghResponseToInstance(
	pr:
		| RestEndpointMethodTypes['pulls']['create']['response']['data']
		| RestEndpointMethodTypes['pulls']['list']['response']['data'][number]
): PullRequest {
	const labels: Label[] = pr.labels?.map((label) => ({
		name: label.name,
		description: label.description || undefined,
		color: label.color
	}));

	return {
		htmlUrl: pr.html_url,
		number: pr.number,
		title: pr.title,
		body: pr.body || undefined,
		author: pr.user
			? {
					name: pr.user.login || undefined,
					email: pr.user.email || undefined,
					isBot: pr.user.type.toLowerCase() === 'bot',
					gravatarUrl: pr.user.avatar_url
				}
			: null,
		labels: labels,
		draft: pr.draft || false,
		createdAt: new Date(pr.created_at),
		modifiedAt: new Date(pr.created_at),
		sourceBranch: pr.head?.ref,
		targetBranch: pr.base?.ref,
		sha: pr.head?.sha,
		mergedAt: pr.merged_at ? new Date(pr.merged_at) : undefined,
		closedAt: pr.closed_at ? new Date(pr.closed_at) : undefined,
		repoOwner: pr.head?.repo?.owner.login,
		repositorySshUrl: pr.head?.repo?.ssh_url,
		repositoryHttpsUrl: pr.head?.repo?.clone_url
	};
}

export type GitHubListCheckSuitesResp =
	RestEndpointMethodTypes['checks']['listSuitesForRef']['response']['data'];
export type GitHubCheckSuites =
	RestEndpointMethodTypes['checks']['listSuitesForRef']['response']['data']['check_suites'];

export function parseGitHubCheckSuites(data: GitHubListCheckSuitesResp): CheckSuite[] {
	const result = data.check_suites.map((checkSuite) => ({
		name: checkSuite.app?.name,
		status: checkSuite.status,
		count: checkSuite.latest_check_runs_count
	}));
	return result;
}

export type GitHubListChecksResp =
	RestEndpointMethodTypes['checks']['listForRef']['response']['data'];

export function parseGitHubCheckRuns(data: GitHubListChecksResp): ChecksStatus | null {
	// Fetch with retries since checks might not be available _right_ after
	// the pull request has been created.

	// If there are no checks then there is no status to report
	const checkRuns = data.check_runs;
	if (checkRuns.length === 0) return null;

	// Establish when the first check started running, useful for showing
	// how long something has been running.
	const starts = checkRuns
		.map((run) => run.started_at)
		.filter((startedAt) => startedAt !== null) as string[];
	const startTimes = starts.map((startedAt) => new Date(startedAt));

	const queued = checkRuns.filter((c) => c.status === 'queued').length;
	const failed = checkRuns.filter((c) => c.conclusion === 'failure').length;
	const skipped = checkRuns.filter((c) => c.conclusion === 'skipped').length;
	const succeeded = checkRuns.filter((c) => c.conclusion === 'success').length;

	const firstStart = new Date(Math.min(...startTimes.map((date) => date.getTime())));
	const completed = checkRuns.every((check) => !!check.completed_at);
	const totalCount = data.total_count;

	const success = queued === 0 && failed === 0 && skipped + succeeded === totalCount;
	const finished = checkRuns.filter(
		(c) => c.conclusion && ['failure', 'success'].includes(c.conclusion)
	).length;

	return {
		startedAt: firstStart,
		hasChecks: !!totalCount,
		success,
		failed,
		completed,
		queued,
		totalCount,
		skipped,
		finished
	};
}

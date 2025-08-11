import { ghQuery } from '$lib/forge/github/ghQuery';
import { type ChecksResult } from '$lib/forge/github/types';
import { eventualConsistencyCheck } from '$lib/forge/shared/progressivePolling';
import { providesItem, ReduxTag } from '$lib/state/tags';
import type { ChecksService } from '$lib/forge/interface/forgeChecksMonitor';
import type { ChecksStatus } from '$lib/forge/interface/types';
import type { QueryOptions } from '$lib/state/butlerModule';
import type { GitHubApi } from '$lib/state/clientState.svelte';

export class GitHubChecksMonitor implements ChecksService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(gitHubApi: GitHubApi) {
		this.api = injectEndpoints(gitHubApi);
	}

	get(branchName: string, options?: QueryOptions) {
		return this.api.endpoints.listChecks.useQuery(
			{ ref: branchName },
			{
				transform: (result) => parseChecks(result),
				...options
			}
		);
	}

	async fetch(branchName: string, options?: QueryOptions) {
		return await this.api.endpoints.listChecks.fetch(
			{ ref: branchName },
			{
				transform: (result) => parseChecks(result),
				...options
			}
		);
	}
}

function hasChecks(data: ChecksResult): boolean {
	return data.check_runs.length > 0;
}

function parseChecks(data: ChecksResult): ChecksStatus | null {
	// Fetch with retries since checks might not be available _right_ after
	// the pull request has been created.

	// If there are no checks then there is no status to report
	if (!hasChecks(data)) return null;

	const checkRuns = data.check_runs;

	// Establish when the first check started running, useful for showing
	// how long something has been running.
	const starts = checkRuns
		.map((run) => run.started_at)
		.filter((startedAt) => startedAt !== null) as string[];
	const startTimes = starts.map((startedAt) => new Date(startedAt));

	const failedChecks = checkRuns.filter((c) => c.conclusion === 'failure');
	const failed = failedChecks.length;
	const actionRequired = checkRuns.filter((c) => c.conclusion === 'action_required').length;

	const firstStart = new Date(Math.min(...startTimes.map((date) => date.getTime())));
	const completed = failed !== 0 || checkRuns.every((check) => !!check.completed_at);

	const success = failed === 0 && completed && actionRequired === 0;

	return {
		startedAt: firstStart.toISOString(),
		success,
		completed,
		failedChecks: failedChecks.map((check) => check.name)
	};
}

function injectEndpoints(api: GitHubApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			listChecks: build.query<ChecksResult, { ref: string }>({
				queryFn: async ({ ref }, api) => {
					async function listChecksForRef() {
						return await ghQuery({
							domain: 'checks',
							action: 'listForRef',
							extra: api.extra,
							parameters: {
								ref
							}
						});
					}

					return eventualConsistencyCheck(listChecksForRef, (response) => {
						if (response.error) {
							return true; // Stop if there's an error
						}
						return hasChecks(response.data);
					});
				},
				providesTags: (_result, _error, args) => [...providesItem(ReduxTag.Checks, args.ref)]
			})
		})
	});
}

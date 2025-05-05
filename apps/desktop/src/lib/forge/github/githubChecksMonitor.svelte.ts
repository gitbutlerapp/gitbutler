import { ghQuery } from '$lib/forge/github/ghQuery';
import { type ChecksResult } from '$lib/forge/github/types';
import { providesItem, ReduxTag } from '$lib/state/tags';
import type { ChecksService } from '$lib/forge/interface/forgeChecksMonitor';
import type { ChecksStatus } from '$lib/forge/interface/types';
import type { QueryOptions } from '$lib/state/butlerModule';
import type { GitHubApi } from '$lib/state/clientState.svelte';

export const MIN_COMPLETED_AGE = 20000;

export class GitHubChecksMonitor implements ChecksService {
	private api: ReturnType<typeof injectEndpoints>;

	constructor(gitHubApi: GitHubApi) {
		this.api = injectEndpoints(gitHubApi);
	}

	get(branchName: string, options?: QueryOptions) {
		const result = $derived(
			this.api.endpoints.listChecks.useQuery(
				{ ref: branchName },
				{
					transform: (result) => parseChecks(result),
					...options
				}
			)
		);
		return result;
	}
}

function parseChecks(data: ChecksResult): ChecksStatus | null {
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

	const failed = checkRuns.filter((c) => c.conclusion === 'failure').length;
	const actionRequired = checkRuns.filter((c) => c.conclusion === 'action_required').length;

	const firstStart = new Date(Math.min(...startTimes.map((date) => date.getTime())));
	const completed = checkRuns.every((check) => !!check.completed_at);

	const success = completed && failed === 0 && actionRequired === 0;

	return {
		startedAt: firstStart.toISOString(),
		success,
		completed
	};
}

function injectEndpoints(api: GitHubApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			listChecks: build.query<ChecksResult, { ref: string }>({
				queryFn: async ({ ref }, api) =>
					await ghQuery({
						domain: 'checks',
						action: 'listForRef',
						extra: api.extra,
						parameters: {
							ref
						}
					}),
				providesTags: (_result, _error, args) => [...providesItem(ReduxTag.Checks, args.ref)]
			})
		})
	});
}

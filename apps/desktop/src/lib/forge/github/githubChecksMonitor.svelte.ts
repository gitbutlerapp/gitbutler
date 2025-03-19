import { ghQuery } from './ghQuery';
import { scurveBackoff } from '$lib/backoff/scurve';
import { type ChecksResult, type SuitesResult } from '$lib/forge/github/types';
import { ReduxTag } from '$lib/state/tags';
import { sleep } from '$lib/utils/sleep';
import { writable } from 'svelte/store';
import type { ChecksStatus } from '$lib/forge/interface/types';
import type { GitHubApi } from '$lib/state/clientState.svelte';
import type { ForgeChecksMonitor } from '../interface/forgeChecksMonitor';

export const MIN_COMPLETED_AGE = 20000;

export class GitHubChecksMonitor implements ForgeChecksMonitor {
	private _status: ChecksStatus | undefined | null;
	readonly status = writable<ChecksStatus | undefined | null>(undefined, () => {
		// Hack: updating the loading writable can lead to state_unsafe_mutation.
		setTimeout(() => {
			this.start();
		}, 0);
		return () => {
			this.stop();
		};
	});
	readonly loading = writable(false);
	readonly error = writable<any>();

	private timeout: any;
	private hasCheckSuites: boolean | undefined;

	private api: ReturnType<typeof injectEndpoints>;

	constructor(
		gitHubApi: GitHubApi,
		private sourceBranch: string
	) {
		this.api = injectEndpoints(gitHubApi);
	}

	async start() {
		this.update();
	}

	stop() {
		if (this.timeout) clearTimeout(this.timeout);
		delete this.timeout;
	}

	async update() {
		this.error.set(undefined);
		this.loading.set(true);

		try {
			const checks = await this.fetchChecksWithRetries(this.sourceBranch, 5, 2000);
			if (checks) {
				const status = parseChecks(checks);
				this.status.set(status);
				this._status = status;
			}
		} catch (e: any) {
			console.error(e);
			this.error.set(e.message);
			if (!e.message?.includes('No commit found')) {
				// toasts.error('Failed to fetch checks');
			}
		} finally {
			this.loading.set(false);
		}

		if (!this.hasCheckSuites) return;

		const delay = this.getNextDelay();
		if (delay) this.timeout = setTimeout(async () => await this.update(), delay);
	}

	getLastStatus() {
		return this._status;
	}

	private getNextDelay() {
		const status = this._status;
		if (!status || !status.startedAt) return;

		const ageMs = Date.now() - status.startedAt.getTime();
		// Only stop polling for updates if checks have completed and check suite age is
		// more than some min age. We do this to work around a bug where just after pushing
		// to a branch GitHub might not report all the checks that will eventually be
		// run as part of the suite.
		if (status.completed && ageMs > MIN_COMPLETED_AGE) return;
		return scurveBackoff(ageMs, 10000, 600000);
	}

	private async fetchChecksWithRetries(ref: string, retries: number, delayMs: number) {
		let checks = await this.fetchChecks(ref);
		if (!checks) {
			return;
		}

		if (checks.total_count > 0) {
			this.hasCheckSuites = true;
			return checks;
		}

		if (this.hasCheckSuites === undefined) {
			const result = await this.getCheckSuites();
			this.hasCheckSuites = (result?.total_count || 0) > 0;
		}

		if (!this.hasCheckSuites) return checks;

		let attempts = 0;
		while (checks?.total_count === 0 && attempts < retries) {
			attempts++;
			await sleep(delayMs);
			checks = await this.fetchChecks(ref);
		}
		return checks;
	}

	private async getCheckSuites() {
		const result = await this.api.endpoints.listSuites.fetch(this.sourceBranch, {
			forceRefetch: true
		});
		return result.data;
	}

	private async fetchChecks(ref: string) {
		const result = await this.api.endpoints.listChecks.fetch(ref, { forceRefetch: true });
		return result.data;
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

	const queued = checkRuns.filter((c) => c.status === 'queued').length;
	const failed = checkRuns.filter((c) => c.conclusion === 'failure').length;
	const actionRequired = checkRuns.filter((c) => c.conclusion === 'action_required').length;

	const firstStart = new Date(Math.min(...startTimes.map((date) => date.getTime())));
	const completed = checkRuns.every((check) => !!check.completed_at);

	const success = queued === 0 && failed === 0 && actionRequired === 0;

	return {
		startedAt: firstStart,
		success,
		completed
	};
}

function injectEndpoints(api: GitHubApi) {
	return api.injectEndpoints({
		endpoints: (build) => ({
			listChecks: build.query<ChecksResult, string>({
				queryFn: async (ref, api) =>
					await ghQuery({
						domain: 'checks',
						action: 'listForRef',
						extra: api.extra,
						parameters: {
							ref
						}
					}),
				providesTags: [ReduxTag.PullRequests]
			}),
			listSuites: build.query<SuitesResult, string>({
				queryFn: async (ref, api) =>
					await ghQuery({
						domain: 'checks',
						action: 'listSuitesForRef',
						extra: api.extra,
						parameters: {
							ref
						}
					}),
				providesTags: [ReduxTag.PullRequests]
			})
		})
	});
}

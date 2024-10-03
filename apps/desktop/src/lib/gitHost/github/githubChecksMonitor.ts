import { scurveBackoff } from '$lib/backoff/scurve';
import { DEFAULT_HEADERS } from '$lib/gitHost/github/headers';
import { parseGitHubCheckSuites } from '$lib/gitHost/github/types';
import { sleep } from '$lib/utils/sleep';
import { Octokit, type RestEndpointMethodTypes } from '@octokit/rest';
import { writable } from 'svelte/store';
import type { CheckSuites, ChecksStatus } from '$lib/gitHost/interface/types';
import type { RepoInfo } from '$lib/url/gitUrl';
import type { GitHostChecksMonitor } from '../interface/gitHostChecksMonitor';

export const MIN_COMPLETED_AGE = 20000;

export class GitHubChecksMonitor implements GitHostChecksMonitor {
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

	constructor(
		private octokit: Octokit,
		private repo: RepoInfo,
		private sourceBranch: string
	) {}

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
			const status = parseChecks(checks);
			this.status.set(status);
			this._status = status;
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
		if (checks.total_count > 0) {
			this.hasCheckSuites = true;
			return checks;
		}

		if (this.hasCheckSuites === undefined) {
			const suites = await this.getCheckSuites();
			this.hasCheckSuites = suites.count > 0;
		}

		if (!this.hasCheckSuites) return checks;

		let attempts = 0;
		while (checks.total_count === 0 && attempts < retries) {
			attempts++;
			await sleep(delayMs);
			checks = await this.fetchChecks(ref);
		}
		return checks;
	}

	private async getCheckSuites(): Promise<CheckSuites> {
		const resp = await this.octokit.checks.listSuitesForRef({
			headers: DEFAULT_HEADERS,
			owner: this.repo.owner,
			repo: this.repo.name,
			ref: this.sourceBranch
		});
		return { count: resp.data.total_count, items: parseGitHubCheckSuites(resp.data) };
	}

	private async fetchChecks(ref: string) {
		const resp = await this.octokit.checks.listForRef({
			headers: DEFAULT_HEADERS,
			owner: this.repo.owner,
			repo: this.repo.name,
			ref: ref
		});
		return resp.data;
	}
}

function parseChecks(
	data: RestEndpointMethodTypes['checks']['listForRef']['response']['data']
): ChecksStatus | null {
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

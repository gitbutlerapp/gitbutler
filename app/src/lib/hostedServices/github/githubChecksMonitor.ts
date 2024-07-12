import { getBackoffByAge } from '../backoff/backoff';
import { DEFAULT_HEADERS } from '$lib/hostedServices/github/headers';
import { parseGitHubCheckSuites } from '$lib/hostedServices/github/types';
import { sleep } from '$lib/utils/sleep';
import { get, writable } from 'svelte/store';
import type { CheckSuites, ChecksStatus } from '$lib/hostedServices/interface/types';
import type { RepoInfo } from '$lib/url/gitUrl';
import type { HostedGitChecksMonitor } from '../interface/hostedGitChecksMonitor';
import type { Octokit } from '@octokit/rest';

export class GitHubChecksMonitor implements HostedGitChecksMonitor {
	readonly result = writable<ChecksStatus | undefined | null>(undefined, () => {
		this.fetch();
		return () => {
			this.stop();
		};
	});

	readonly loading = writable(false);
	readonly error = writable();

	timeout: any;

	constructor(
		private octokit: Octokit,
		private repo: RepoInfo,
		private sourceBranch: string
	) {}

	refresh() {
		this.fetch();
	}

	private async fetch() {
		this.error.set(undefined);
		this.loading.set(true);
		try {
			this.result.set(await this.checks(this.sourceBranch));
		} catch (e: any) {
			console.error(e);
			this.error.set(e.message);
			if (!e.message.includes('No commit found')) {
				// toasts.error('Failed to fetch checks');
			}
		} finally {
			this.loading.set(false);
		}

		const delay = this.getNextDelay();
		if (delay) this.timeout = setTimeout(async () => await this.fetch(), delay);
	}

	private stop() {
		if (this.timeout) clearTimeout(this.timeout);
		delete this.timeout;
	}

	private getNextDelay() {
		const checks = get(this.result);
		if (!checks || !checks.startedAt) return;

		const msAgo = Date.now() - checks.startedAt.getTime();
		// Only stop polling for updates if checks have completed and check suite age is
		// more than a minute. We do this to work around a bug where just after pushing
		// to a branch GitHub might not report all the checks that will eventually be
		// run as part of the suite.
		if (checks?.completed && msAgo > 60000) return;

		const backoff = getBackoffByAge(msAgo);
		return backoff;
	}

	private async checks(ref: string | undefined): Promise<ChecksStatus | null> {
		if (!ref) return null;

		// Fetch with retries since checks might not be available _right_ after
		// the pull request has been created.
		const resp = await this.fetchChecksWithRetries(ref, 5, 2000);

		// If there are no checks then there is no status to report
		const checks = resp.data.check_runs;
		if (checks.length === 0) return null;

		// Establish when the first check started running, useful for showing
		// how long something has been running.
		const starts = resp.data.check_runs
			.map((run) => run.started_at)
			.filter((startedAt) => startedAt !== null) as string[];
		const startTimes = starts.map((startedAt) => new Date(startedAt));

		const queued = checks.filter((c) => c.status === 'queued').length;
		const failed = checks.filter((c) => c.conclusion === 'failure').length;
		const skipped = checks.filter((c) => c.conclusion === 'skipped').length;
		const succeeded = checks.filter((c) => c.conclusion === 'success').length;

		const firstStart = new Date(Math.min(...startTimes.map((date) => date.getTime())));
		const completed = checks.every((check) => !!check.completed_at);
		const totalCount = resp?.data.total_count;

		const success = queued === 0 && failed === 0 && skipped + succeeded === totalCount;
		const finished = checks.filter(
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

	private async fetchChecksWithRetries(ref: string, retries: number, delayMs: number) {
		let resp = await this.fetchChecks(ref);
		let retried = 0;
		let shouldWait: boolean | undefined = undefined;

		while (resp.data.total_count === 0 && retried < retries) {
			if (shouldWait === undefined && retried === 0) {
				shouldWait = await this.shouldWaitForChecks(ref);
				if (!shouldWait) {
					return resp;
				}
			}
			await sleep(delayMs);
			resp = await this.fetchChecks(ref);
			retried++;
		}
		return resp;
	}

	private async shouldWaitForChecks(ref: string) {
		const resp = await this.getCheckSuites(ref);
		const checkSuites = resp?.items;
		if (!checkSuites) return true;

		// Continue waiting if some check suites are in progress
		if (checkSuites.some((suite) => suite.status !== 'completed')) return true;
	}

	private async getCheckSuites(ref: string | undefined): Promise<CheckSuites> {
		if (!ref) return null;
		const resp = await this.octokit.checks.listSuitesForRef({
			headers: DEFAULT_HEADERS,
			owner: this.repo.owner,
			repo: this.repo.name,
			ref: ref
		});
		return { count: resp.data.total_count, items: parseGitHubCheckSuites(resp.data) };
	}

	private async fetchChecks(ref: string) {
		return await this.octokit.checks.listForRef({
			headers: DEFAULT_HEADERS,
			owner: this.repo.owner,
			repo: this.repo.name,
			ref: ref
		});
	}
}

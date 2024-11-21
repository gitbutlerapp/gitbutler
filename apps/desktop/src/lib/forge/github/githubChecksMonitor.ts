import { scurveBackoff } from '$lib/backoff/scurve';
import { DEFAULT_HEADERS } from '$lib/forge/github/headers';
import { parseGitHubCheckRuns, parseGitHubCheckSuites } from '$lib/forge/github/types';
import { sleep } from '$lib/utils/sleep';
import { LocalCache } from '@gitbutler/shared/localCache';
import { Octokit } from '@octokit/rest';
import { writable } from 'svelte/store';
import type { CheckSuites, ChecksStatus } from '$lib/forge/interface/types';
import type { RepoInfo } from '$lib/url/gitUrl';
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

	// Stores previously fetched checks results by pr number.
	private cache = new LocalCache({ keyPrefix: 'pr-checks', expiry: 1440 });

	constructor(
		private octokit: Octokit,
		private repo: RepoInfo,
		private sourceBranch: string
	) {}

	async start() {
		this.loadFromCache();
		this.update();
	}

	stop() {
		if (this.timeout) clearTimeout(this.timeout);
		delete this.timeout;
	}

	loadFromCache() {
		const cachedValue = this.cache.get(this.sourceBranch);
		try {
			const status = parseGitHubCheckRuns(cachedValue);
			this._status = status;
			this.status.set(status);
		} catch {
			this.cache.remove(this.sourceBranch);
		}
	}

	async update() {
		this.error.set(undefined);
		this.loading.set(true);

		try {
			const checks = await this.fetchChecksWithRetries(this.sourceBranch, 5, 2000);
			this.status.set(checks);
			this._status = checks;
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

	private async fetchChecksWithRetries(
		ref: string,
		retries: number,
		delayMs: number
	): Promise<ChecksStatus | null> {
		let checks = await this.fetchChecks(ref);
		if (checks && checks?.totalCount > 0) {
			this.hasCheckSuites = true;
			return checks;
		}

		if (this.hasCheckSuites === undefined) {
			const suites = await this.getCheckSuites();
			this.hasCheckSuites = suites.count > 0;
		}

		if (!this.hasCheckSuites) return checks;

		let attempts = 0;
		while (checks && checks.totalCount === 0 && attempts < retries) {
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

	private async fetchChecks(ref: string): Promise<ChecksStatus | null> {
		const resp = await this.octokit.checks.listForRef({
			headers: DEFAULT_HEADERS,
			owner: this.repo.owner,
			repo: this.repo.name,
			ref: ref
		});
		this.cache.set(ref, resp.data);
		return parseGitHubCheckRuns(resp.data);
	}
}

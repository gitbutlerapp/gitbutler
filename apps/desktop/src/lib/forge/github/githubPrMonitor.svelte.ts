import { type ForgePrMonitor } from '$lib/forge/interface/forgePrMonitor';
import { type DetailedPullRequest } from '$lib/forge/interface/types';
import { sleep } from '$lib/utils/sleep';
import { derived, writable } from 'svelte/store';
import type { ForgePrService } from '$lib/forge/interface/forgePrService';

export const PR_SERVICE_INTERVAL = 120 * 60 * 1000;
const MAX_POLL_ATTEMPTS = 6;

export class GitHubPrMonitor implements ForgePrMonitor {
	readonly pr = writable<DetailedPullRequest | undefined>(undefined, () => {
		this.start();
		return () => {
			this.stop();
		};
	});

	readonly loading = writable(false);
	readonly error = writable<any>();

	readonly mergeableState = derived(this.pr, (pr) => pr?.mergeableState);

	private intervalId: any;

	constructor(
		private prService: ForgePrService,
		private prNumber: number
	) {}

	private start() {
		this.fetch();
		this.intervalId = setInterval(() => {
			this.fetch();
		}, PR_SERVICE_INTERVAL);
	}

	private stop() {
		if (this.intervalId) clearInterval(this.intervalId);
	}

	async refresh(): Promise<void> {
		await this.fetch();
	}

	private async fetch() {
		this.error.set(undefined);
		this.loading.set(true);
		try {
			await this.loadPrWithRetries(this.prNumber);
		} catch (err: any) {
			this.error.set(err);
			console.error(err);
		} finally {
			this.loading.set(false);
		}
	}

	/**
	 * Loads pull request details.
	 *
	 * Right after pushing GitHub will respond with status 422, necessatating
	 * retries. After that, it can take a few seconds for the mergeable state
	 * to be known.
	 */
	private async loadPrWithRetries(prNumber: number): Promise<void> {
		const request = async () => {
			return await this.prService.fetch(prNumber);
		};
		let lastError: any;
		let attempt = 0;
		while (attempt++ < MAX_POLL_ATTEMPTS) {
			try {
				const prResult = await request();
				const pr = prResult.data;
				this.pr.set(pr);
				if (!pr) {
					continue;
				}
				if (pr.mergeableState !== 'unknown' || pr.merged) {
					return;
				}

				// Stop polling polling if merged or mergeable state known.
			} catch (err: any) {
				if (err.status !== 422) throw err;
				lastError = err;
			}
			await sleep(1000);
		}
		if (lastError) {
			throw lastError;
		}
		// The end of the function is reached if the pull request has an
		// unknown mergeable state after retries.
	}
}

import { type DetailedPullRequest } from '$lib/forge/interface/types';
import { sleep } from '$lib/utils/sleep';
import { derived, writable } from 'svelte/store';
import type { GitHubPrService } from './githubPrService';
import type { ForgePrMonitor } from '../interface/forgePrMonitor';

export const PR_SERVICE_INTERVAL = 20 * 60 * 1000;
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
	readonly lastFetch = writable<Date | undefined>();

	private intervalId: any;

	constructor(
		private prService: GitHubPrService,
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
			this.pr.set(await this.getPrWithRetries(this.prNumber));
			this.lastFetch.set(new Date());
		} catch (err: any) {
			this.error.set(err);
			console.error(err);
		} finally {
			this.loading.set(false);
		}
	}

	/**
	 * Get the PR information.
	 *
	 * Right after pushing GitHub will respond with status 422, necessatating retries.
	 * After that, the mergeable state is not always available immediately.
	 * The function will set the initial PR information immediately and then poll for the mergeable state.
	 */
	private async getPrWithRetries(prNumber: number): Promise<DetailedPullRequest> {
		const request = async () => await this.prService.get(prNumber);
		let lastError: any;
		let attempt = 0;
		while (attempt < MAX_POLL_ATTEMPTS) {
			attempt++;
			try {
				const detailedPR = await request();
				// Set the PR info immediately to show
				// the user all preliminary information.
				this.pr.set(detailedPR);
				if (detailedPR.mergeableState === 'unknown') {
					await sleep(1000);
					continue;
				}
				return detailedPR;
			} catch (err: any) {
				if (err.status !== 422) throw err;
				lastError = err;
				await sleep(1000);
			}
		}
		throw lastError;
	}
}

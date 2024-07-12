import { type DetailedPullRequest } from '$lib/hostedServices/interface/types';
import { sleep } from '$lib/utils/sleep';
import { derived, writable } from 'svelte/store';
import type { GitHubPrService } from './githubPrService';
import type { HostedGitPrMonitor } from '../interface/hostedGitPrMonitor';

export class GitHubPrMonitor implements HostedGitPrMonitor {
	readonly pr = writable<DetailedPullRequest | undefined>(undefined, () => {
		this.fetch();
		return () => {
			this.stop();
		};
	});

	readonly loading = writable(false);
	readonly error = writable();

	readonly mergeableState = derived(this.pr, (pr) => pr?.mergeableState);
	readonly lastFetch = writable<Date | undefined>();

	private timeoutId: any;

	constructor(
		private prService: GitHubPrService,
		private prNumber: number
	) {}

	async refresh(): Promise<void> {
		await this.fetch();
	}

	private stop() {
		if (this.timeoutId) clearTimeout(this.timeoutId);
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

	async getPrWithRetries(prNumber: number): Promise<DetailedPullRequest | undefined> {
		// Right after pushing GitHub will respond with status 422, retries are necessary
		const attempt = 0;
		const request = async () => {
			return await this.prService.get(prNumber);
		};
		const pr = await request();

		while (!pr && attempt < 3) {
			try {
				return await request();
			} catch (err: any) {
				if (err.status !== 422) throw err;
				await sleep(1000);
			}
		}
		if (!pr) throw 'Failed to fetch pull request details';
		return pr;
	}
}

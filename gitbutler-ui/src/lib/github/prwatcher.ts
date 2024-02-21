import { sleep } from '$lib/utils/sleep';
import type { Octokit } from '@octokit/rest';
import { Observable, exhaustMap, from, interval, map, mergeMap, takeWhile, timer } from 'rxjs';

const url = 'your_api_endpoint_here';

const maxRequests = 60;
const initialDelaySeconds = 30;
const subsequentDelaySeconds = 60;

export type PrStatus = {
	mergedAt: string | null;
	number: number;
};

export class PrWatcher {
	get$: Observable<PrStatus>;

	constructor(
		private pullNumber: number,
		private repo: string,
		private owner: string,
		private octokit: Octokit
	) {
		this.get$ = interval(1000).pipe(
			// Ignores interval until inner observable completes.
			exhaustMap((attempt) => this.fetchData(attempt + 1)),
			takeWhile((response) => !response.data.merged, true),
			map((resp) => {
				const data = resp.data;
				return { mergedAt: data.merged_at, number: data.number };
			})
		);
	}

	async fetchData(attempt: number) {
		console.log('Fetching attempt: ' + attempt);
		const delay = attempt <= 10 ? initialDelaySeconds : subsequentDelaySeconds;
		await sleep(delay);
		return await this.queryUrl();
	}

	async queryUrl() {
		return await this.octokit.pulls.get({
			owner: this.owner,
			repo: this.repo,
			pull_number: this.pullNumber
		});
	}
}

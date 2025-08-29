import { sleep } from '$lib/utils/sleep';

const MAX_ATTEMPTS = 2;
const INITIAL_DELAY = 2000; // 2 seconds

/**
 * Call a function that returns a promise and check its result repeatedly.
 *
 * Repeatedly calls `promiseFn` until `shouldStop` returns true or max attempts reached.
 * Uses exponential backoff for delays.
 */
export async function eventualConsistencyCheck<T>(
	promiseFn: () => Promise<T>,
	shouldStop: (r: T) => boolean
): Promise<T> {
	let result: T = await promiseFn();
	for (let attempts = 0; !shouldStop(result) && attempts < MAX_ATTEMPTS; attempts++) {
		await sleep(INITIAL_DELAY * Math.pow(2, attempts));
		result = await promiseFn();
	}
	return result;
}

const POLLING_INTERVAL_INITIAL = 5 * 1000;
const POLLING_INTERVAL_SHORT = 30 * 1000;
const POLLING_INTERVAL_MEDIUM = 5 * 60 * 1000;
const POLLING_INTERVAL_LONG = 30 * 60 * 1000;

const POLLING_THRESHOLD_INITIAL = 60 * 1000;
const POLLING_THRESHOLD_SHORT = 10 * 60 * 1000;
const POLLING_THRESHOLD_MEDIUM = 60 * 60 * 1000;

export function getPollingInterval(elapsedMs: number, shouldStop: boolean): number {
	if (shouldStop) {
		return 0; // Stop polling
	}

	if (elapsedMs < POLLING_THRESHOLD_INITIAL) {
		return POLLING_INTERVAL_INITIAL;
	}

	if (elapsedMs < POLLING_THRESHOLD_SHORT) {
		return POLLING_INTERVAL_SHORT;
	}

	if (elapsedMs < POLLING_THRESHOLD_MEDIUM) {
		return POLLING_INTERVAL_MEDIUM;
	}

	return POLLING_INTERVAL_LONG;
}

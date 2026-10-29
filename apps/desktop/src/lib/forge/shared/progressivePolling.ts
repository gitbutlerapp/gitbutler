const POLLING_INTERVAL_INITIAL = 5 * 1000;
const POLLING_INTERVAL_SHORT = 30 * 1000;
const POLLING_INTERVAL_MEDIUM = 5 * 60 * 1000;
const POLLING_INTERVAL_LONG = 30 * 60 * 1000;

const POLLING_THRESHOLD_INITIAL = 60 * 1000;
const POLLING_THRESHOLD_SHORT = 10 * 60 * 1000;
const POLLING_THRESHOLD_MEDIUM = 60 * 60 * 1000;

/**
 * Pick a polling interval, backing off progressively as consecutive polls fail.
 *
 * `consecutiveErrors` is 0 while the query is healthy, 1 after the first failed
 * poll, and 2+ once failures persist. A failing endpoint (offline, rate limited,
 * repo access lost) is stepped out to a short interval on the first blip — so a
 * one-off failure barely slows polling — and to a medium interval once it keeps
 * failing, so we stop hammering it. The backoff is a floor: it never polls
 * *faster* than the normal schedule (e.g. an error during the slow long-lived
 * schedule does not speed polling up). A successful fetch resets the count and
 * restores the progressive schedule.
 */
export function getPollingInterval(
	elapsedMs: number,
	shouldStop: boolean,
	consecutiveErrors: number = 0,
): number {
	if (shouldStop) {
		return 0; // Stop polling
	}

	const normal = normalPollingInterval(elapsedMs);

	if (consecutiveErrors >= 2) {
		return Math.max(normal, POLLING_INTERVAL_MEDIUM);
	}
	if (consecutiveErrors >= 1) {
		return Math.max(normal, POLLING_INTERVAL_SHORT);
	}
	return normal;
}

/** The progressive interval for a healthy query, widening with elapsed time. */
function normalPollingInterval(elapsedMs: number): number {
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

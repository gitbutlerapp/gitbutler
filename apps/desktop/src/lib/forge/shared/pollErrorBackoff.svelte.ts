import { classify } from "$lib/error/errorClassification";
import { getPollingInterval } from "$lib/forge/shared/progressivePolling";

/** The bits of a query result the backoff cares about. */
type PollResult = { isError?: boolean; error?: unknown; startedTimeStamp?: number } | undefined;

/**
 * Progressive polling that backs off as a query keeps failing.
 *
 * `pollingInterval` follows the normal progressive schedule while the query is
 * healthy, steps out after the first failed poll, and steps out further once
 * failures persist (offline, rate-limited, repo access lost) — see
 * {@link getPollingInterval}. A successful fetch (including refetch-on-focus /
 * reconnect or a manual retry) resets the count and restores the schedule.
 *
 * An error whose classification is `terminal` (a PAT missing a permission,
 * an org OAuth block, no forge credentials) stops automatic polling entirely
 * — no interval will fix it. Refetch-on-focus and manual retries still
 * probe, and a success clears the stop.
 *
 * The failure count is `$state` written from an `$effect`, not a `$derived` off
 * the query: `pollingInterval` feeds the query's own subscription, so deriving
 * the error straight back out of the query would form a reactive cycle. The
 * writes are guarded so the effect converges (each poll bumps the count at most
 * once) rather than relying on value equality to stop re-running.
 *
 * `getResult` returns the reactive query result, or `undefined` when the query
 * is disabled — treated as "not failing".
 */
export function createPollBackoff(deps: {
	getResult: () => PollResult;
	getElapsedMs: () => number;
	getShouldStop: () => boolean;
}) {
	let consecutiveErrors = $state(0);
	let terminalError = $state(false);
	// Not reactive: just remembers which poll we last counted, so a re-running
	// effect doesn't double-count a single failed request.
	let lastPolledStamp: number | undefined = undefined;

	$effect(() => {
		const result = deps.getResult();
		const errored = result?.isError ?? false;
		const stamp = result?.startedTimeStamp;

		if (!errored) {
			// A healthy (or absent) result clears the backoff.
			if (consecutiveErrors !== 0) consecutiveErrors = 0;
			if (terminalError) terminalError = false;
			lastPolledStamp = stamp;
			return;
		}

		const terminal = result?.error ? (classify(result.error).terminal ?? false) : false;
		if (terminal !== terminalError) terminalError = terminal;

		if (consecutiveErrors === 0) {
			// First failed poll: step out to the short interval.
			consecutiveErrors = 1;
			lastPolledStamp = stamp;
		} else if (stamp !== lastPolledStamp) {
			// A later, distinct poll is still failing: escalate to medium. If the
			// backend doesn't expose a per-poll stamp we simply stay at the short
			// interval, which is still a safe backoff.
			lastPolledStamp = stamp;
			if (consecutiveErrors < 2) consecutiveErrors = 2;
		}
	});

	return {
		get pollingInterval() {
			return getPollingInterval(
				deps.getElapsedMs(),
				deps.getShouldStop() || terminalError,
				consecutiveErrors,
			);
		},
	};
}

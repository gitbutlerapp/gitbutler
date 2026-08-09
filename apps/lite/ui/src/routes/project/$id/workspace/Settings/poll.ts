/** Sleeps, unless the signal aborts first. */
const wait = (ms: number, signal: AbortSignal): Promise<void> =>
	new Promise((resolve, reject) => {
		if (signal.aborted) {
			reject(signal.reason);
			return;
		}
		// Dropped once the timer wins, so a long poll does not pile listeners onto one signal.
		const onAbort = () => {
			clearTimeout(timer);
			reject(signal.reason);
		};
		const timer = setTimeout(() => {
			signal.removeEventListener("abort", onAbort);
			resolve();
		}, ms);
		signal.addEventListener("abort", onAbort, { once: true });
	});

/**
 * Retries `attempt` until it succeeds, the deadline passes, or the signal aborts.
 *
 * Both browser sign-ins finish out of band, so asking repeatedly is the only way to learn
 * they did. The signal matters as much as the deadline: these run for minutes, and a flow
 * the user walked away from would otherwise keep calling the backend to the end of it.
 *
 * The interval is waited before the first attempt, not after: the browser has only just
 * been opened, so an immediate call cannot succeed, and GitHub's device flow asks not to
 * be polled faster than its interval from the very first request.
 */
export const pollUntilSuccess = async ({
	attempt,
	intervalMs,
	timeoutMs,
	signal,
	isRetryable = () => true,
}: {
	attempt: () => Promise<unknown>;
	intervalMs: number;
	timeoutMs: number;
	signal: AbortSignal;
	/**
	 * Whether a failure is worth another round. Defaults to retrying everything, which
	 * suits a flow whose only signal is "not yet"; a forge that names its refusals should
	 * say so, rather than spend the whole timeout asking again.
	 */
	isRetryable?: (error: unknown) => boolean;
}): Promise<void> => {
	const deadline = Date.now() + timeoutMs;
	for (;;) {
		await wait(intervalMs, signal);
		try {
			await attempt();
			return;
		} catch (error) {
			// Still pending until the user finishes in the browser; past the deadline it is real.
			if (!isRetryable(error) || Date.now() >= deadline) throw error;
		}
	}
};

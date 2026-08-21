import { useEffect, useState } from "react";

/**
 * The current time, re-read every `intervalMs`. Every consumer re-renders on
 * the tick, so use it only where a ticking clock is needed; `null` holds the
 * value instead.
 */
export const useNow = (intervalMs: number | null): number => {
	const [now, setNow] = useState(() => Date.now());

	useEffect(() => {
		if (intervalMs === null) return;

		// The held value can be arbitrarily stale, so the first read cannot wait
		// for the interval. Scheduled, not read here: the clock is impure during
		// render, and setting state in an effect body cascades a render.
		const first = setTimeout(() => setNow(Date.now()));
		const id = setInterval(() => setNow(Date.now()), intervalMs);
		return () => {
			clearTimeout(first);
			clearInterval(id);
		};
	}, [intervalMs]);

	return now;
};

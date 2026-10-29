import { useEffect, useState } from "react";

/**
 * One clock per interval, shared by every subscriber: a page of relative
 * timestamps costs one timer, and its ticks land in one batched render
 * rather than as many staggered ones.
 */
const tickers = new Map<
	number,
	{ timer: ReturnType<typeof setInterval>; listeners: Set<(now: number) => void> }
>();

const subscribeTick = (intervalMs: number, listener: (now: number) => void): (() => void) => {
	let ticker = tickers.get(intervalMs);
	if (ticker === undefined) {
		const listeners = new Set<(now: number) => void>();
		const tick = () => {
			const now = Date.now();
			for (const entry of listeners) entry(now);
		};
		ticker = { timer: setInterval(tick, intervalMs), listeners };
		tickers.set(intervalMs, ticker);
	}
	ticker.listeners.add(listener);
	return () => {
		ticker.listeners.delete(listener);
		if (ticker.listeners.size === 0) {
			clearInterval(ticker.timer);
			tickers.delete(intervalMs);
		}
	};
};

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
		const unsubscribe = subscribeTick(intervalMs, setNow);
		return () => {
			clearTimeout(first);
			unsubscribe();
		};
	}, [intervalMs]);

	return now;
};

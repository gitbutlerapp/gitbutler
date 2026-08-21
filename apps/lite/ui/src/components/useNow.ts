import { useEffect, useState } from "react";

/**
 * The current time, re-read every `intervalMs` — for labels that age while on
 * screen. Every consumer re-renders on the tick, so mount it only where a
 * ticking clock is genuinely needed; pass `null` to hold the mount-time value.
 */
export const useNow = (intervalMs: number | null): number => {
	const [now, setNow] = useState(() => Date.now());

	useEffect(() => {
		if (intervalMs === null) return;

		const id = setInterval(() => setNow(Date.now()), intervalMs);
		return () => clearInterval(id);
	}, [intervalMs]);

	return now;
};

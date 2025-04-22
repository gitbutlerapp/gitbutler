type RateLimitedFunction<T extends (...args: any[]) => any> = (
	...args: Parameters<T>
) => ReturnType<T> | void;

// A rate limiting function that can prevent a function being called more than
// a certain number of times within the given sliding time window.
export function rateLimit<T extends (...args: any[]) => any>(params: {
	// Function to rate-limit.
	fn: T;
	// A name surfaced when limit exceeded.
	name: string;
	// Number of calls permitted within time window.
	limit: number;
	// Window length in milliseconds.
	windowMs: number;
}): RateLimitedFunction<T> {
	const { fn, name, limit, windowMs } = params;
	const timestamps: number[] = [];

	return (...args: Parameters<T>) => {
		const now = Date.now();

		// Remove timestamps older than the time window
		while (timestamps.length > 0 && timestamps[0]! <= now - windowMs) {
			timestamps.shift();
		}

		if (timestamps.length < limit) {
			timestamps.push(now);
			return fn(...args);
		} else {
			throw new Error(`Rate limit for ${name} exceeded, ${limit} / ${windowMs}ms.`);
		}
	};
}

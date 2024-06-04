/**
 * Throttles the execution of a function.
 * @param fn The function to be throttled.
 * @param wait The time to wait between function invocations in milliseconds.
 * @returns A throttled version of the original function.
 */
export function throttle<T extends (...args: any[]) => any>(
	fn: T,
	wait: number
): (...args: Parameters<T>) => ReturnType<T> {
	let inThrottle: boolean;
	let lastResult: ReturnType<T>;

	return function (...args: Parameters<T>) {
		if (!inThrottle) {
			inThrottle = true;
			lastResult = fn(...args);
			setTimeout(() => {
				inThrottle = false;
			}, wait);
		}
		return lastResult;
	};
}

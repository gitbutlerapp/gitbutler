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

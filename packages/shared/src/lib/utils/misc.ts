export function throttle<T extends (...args: any[]) => any>(
	func: T,
	limit: number
): (...args: Parameters<T>) => void {
	let inThrottle: boolean = false;

	return function (this: any, ...args: Parameters<T>) {
		if (!inThrottle) {
			func.apply(this, args);
			inThrottle = true;
			setTimeout(() => {
				inThrottle = false;
			}, limit);
		}
	};
}

export function throttlePromise<T extends (...args: any[]) => Promise<any>>(
	func: T,
	limit: number
): (...args: Parameters<T>) => Promise<void> {
	let inThrottle: boolean = false;

	return async function (this: any, ...args: Parameters<T>) {
		if (!inThrottle) {
			await func.apply(this, args);
			inThrottle = true;
			setTimeout(() => {
				inThrottle = false;
			}, limit);
		}
	};
}

export function debouncePromise<T extends (...args: any[]) => Promise<any>>(
	fn: T,
	delay: number
): (...args: Parameters<T>) => Promise<void> {
	let timeout: ReturnType<typeof setTimeout> | undefined;

	return async function (this: any, ...args: Parameters<T>) {
		return await new Promise<void>((resolve) => {
			clearTimeout(timeout);
			timeout = setTimeout(async () => {
				await fn.apply(this, args);
				resolve();
			}, delay);
		});
	};
}

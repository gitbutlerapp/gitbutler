export function devLog(...args: any[]) {
	if (import.meta.env.MODE === 'development') {
		// eslint-disable-next-line no-console
		console.log(...args);
	}
}

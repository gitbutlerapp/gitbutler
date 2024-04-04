export function debounce<T extends (...args: unknown[]) => void>(fn: T, delay: number): T {
	let timeout: ReturnType<typeof setTimeout> | undefined;
	return ((...args: unknown[]) => {
		clearTimeout(timeout);
		timeout = setTimeout(() => fn(...args), delay);
	}) as T;
}

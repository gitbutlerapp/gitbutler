export function debounce<Args extends any[], Return, Fn extends (...args: Args) => Return>(
	fn: Fn,
	delay: number
): (...args: Args) => void {
	let timeout: ReturnType<typeof setTimeout> | undefined;
	return (...args: Args) => {
		clearTimeout(timeout);
		timeout = setTimeout(() => fn(...args), delay);
	};
}

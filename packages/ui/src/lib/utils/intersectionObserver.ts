export function intersectionObserver(
	node: Element,
	{
		isDisabled,
		callback,
		options
	}: {
		isDisabled?: boolean;
		callback: (
			entry: IntersectionObserverEntry | undefined,
			observer: IntersectionObserver
		) => void;
		options?: IntersectionObserverInit;
	}
) {
	if (isDisabled) return;

	const observer = new IntersectionObserver(
		([entry], observer) => callback(entry, observer),
		options
	);

	observer.observe(node);

	return {
		destroy() {
			observer.disconnect();
		}
	};
}

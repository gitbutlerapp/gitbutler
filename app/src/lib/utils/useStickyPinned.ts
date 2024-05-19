export function useStickyPinned(
	element: HTMLElement,
	callback: (isPinned: boolean, element: HTMLElement) => void
) {
	const observer = new IntersectionObserver(
		([entry]) => {
			callback(entry.intersectionRatio < 1, element);

			console.log('sticky pinned', element, entry.intersectionRatio);
		},
		{
			threshold: [1]
		}
	);

	observer.observe(element);

	return {
		destroy() {
			observer.disconnect();
		}
	};
}

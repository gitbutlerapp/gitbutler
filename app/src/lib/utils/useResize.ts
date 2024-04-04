export function useResize(element: HTMLElement, callback: (width: number, height: number) => void) {
	const resizeObserver = new ResizeObserver((entries) => {
		for (const entry of entries) {
			const { width, height } = entry.contentRect;

			callback(width, height);
		}
	});

	resizeObserver.observe(element);

	return {
		destroy() {
			resizeObserver.disconnect();
		}
	};
}

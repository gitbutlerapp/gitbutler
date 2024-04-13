export function useResize(
	element: HTMLElement,
	callback: (frame: { width: number; height: number }) => void
) {
	const resizeObserver = new ResizeObserver((entries) => {
		for (const entry of entries) {
			const { inlineSize, blockSize } = entry.borderBoxSize[0];

			callback({
				width: Math.round(inlineSize),
				height: Math.round(blockSize)
			});
		}
	});

	resizeObserver.observe(element);

	return {
		destroy() {
			resizeObserver.disconnect();
		}
	};
}

export function resizeObserver(
	element: HTMLElement,
	callback: (data: { currentTarget: HTMLElement; frame: { width: number; height: number } }) => void
) {
	const resizeObserver = new ResizeObserver((entries) => {
		for (const entry of entries) {
			const { inlineSize = 0, blockSize = 0 } = entry.borderBoxSize[0] ?? {};

			callback({
				currentTarget: element,
				frame: {
					width: Math.round(inlineSize),
					height: Math.round(blockSize)
				}
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

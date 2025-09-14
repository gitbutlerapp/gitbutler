export function isScrollable(element: HTMLElement): boolean {
	const style = window.getComputedStyle(element);
	const overflowX = style.overflowX;
	const overflowY = style.overflowY;

	const hasScrollableOverflow =
		overflowX === 'auto' ||
		overflowX === 'scroll' ||
		overflowY === 'auto' ||
		overflowY === 'scroll';

	const hasOverflowingContent =
		element.scrollHeight > element.clientHeight || element.scrollWidth > element.clientWidth;

	return hasScrollableOverflow || hasOverflowingContent;
}

export function isPositioned(element: HTMLElement): boolean {
	const style = window.getComputedStyle(element);
	const position = style.position;
	return position !== 'relative';
}

// Finds ancestor that can properly contain an absolutely positioned overlay
export function findNearestSuitableAncestor(element: HTMLElement): {
	ancestor: HTMLElement;
	accumulatedLeft: number;
	accumulatedTop: number;
} {
	let current: HTMLElement | null = element;
	let accumulatedLeft = 0;
	let accumulatedTop = 0;

	while (current && current !== document.body) {
		accumulatedLeft += current.offsetLeft;
		accumulatedTop += current.offsetTop;

		const nextParent: HTMLElement | null =
			(current.offsetParent as HTMLElement | null) || current.parentElement;

		if (nextParent && nextParent !== document.body) {
			if (isScrollable(nextParent)) {
				return {
					ancestor: nextParent,
					accumulatedLeft,
					accumulatedTop
				};
			}

			if (isPositioned(nextParent)) {
				const isWideEnough = nextParent.offsetWidth > accumulatedLeft + element.offsetWidth;
				const isTallEnough = nextParent.offsetHeight > accumulatedTop + element.offsetHeight;

				if (isWideEnough && isTallEnough) {
					return {
						ancestor: nextParent,
						accumulatedLeft,
						accumulatedTop
					};
				}
			}
		}

		current = nextParent;
	}

	return {
		ancestor: document.body,
		accumulatedLeft,
		accumulatedTop
	};
}

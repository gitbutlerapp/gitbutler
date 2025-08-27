interface ScrollIntoViewIfNeededOptions {
	/**
	 * Whether to center the element in the viewport when scrolling
	 * @default false
	 */
	centerIfNeeded?: boolean;

	/**
	 * The container element to scroll within
	 * If not provided, will scroll the window/document
	 */
	scrollingElement?: Element | null;

	/**
	 * Scroll behavior - 'smooth' for animated scrolling, 'auto' for instant
	 * @default 'auto'
	 */
	behavior?: ScrollBehavior;
}

/**
 * Scrolls an element into view only if it's not already visible in the viewport
 * @param element - The element to scroll into view
 * @param options - Configuration options
 */
export function scrollIntoViewIfNeeded(
	element: Element,
	options: ScrollIntoViewIfNeededOptions = {}
): void {
	const { centerIfNeeded = false, scrollingElement = null, behavior = 'auto' } = options;

	// Get the container that will be scrolled
	const container = scrollingElement || document.documentElement || document.body;
	const isWindow = container === document.documentElement || container === document.body;

	// Get element and container boundaries
	const elementRect = element.getBoundingClientRect();
	const containerRect = isWindow
		? { top: 0, left: 0, bottom: window.innerHeight, right: window.innerWidth }
		: container.getBoundingClientRect();

	// Check if element is already fully visible
	const isFullyVisible =
		elementRect.top >= containerRect.top &&
		elementRect.left >= containerRect.left &&
		elementRect.bottom <= containerRect.bottom &&
		elementRect.right <= containerRect.right;

	// If already fully visible, no need to scroll
	if (isFullyVisible) {
		return;
	}

	// Calculate scroll positions
	let scrollTop: number;
	let scrollLeft: number;

	if (centerIfNeeded) {
		// Center the element in the viewport
		const containerHeight = containerRect.bottom - containerRect.top;
		const containerWidth = containerRect.right - containerRect.left;

		scrollTop = elementRect.top + elementRect.height / 2 - containerHeight / 2;
		scrollLeft = elementRect.left + elementRect.width / 2 - containerWidth / 2;
	} else {
		// Scroll just enough to make the element visible
		const currentScrollTop = isWindow ? window.pageYOffset : container.scrollTop;
		const currentScrollLeft = isWindow ? window.pageXOffset : container.scrollLeft;

		scrollTop = currentScrollTop;
		scrollLeft = currentScrollLeft;

		// Adjust vertical position if needed
		if (elementRect.top < containerRect.top) {
			// Element is above viewport
			scrollTop = currentScrollTop + (elementRect.top - containerRect.top);
		} else if (elementRect.bottom > containerRect.bottom) {
			// Element is below viewport
			scrollTop = currentScrollTop + (elementRect.bottom - containerRect.bottom);
		}

		// Adjust horizontal position if needed
		if (elementRect.left < containerRect.left) {
			// Element is to the left of viewport
			scrollLeft = currentScrollLeft + (elementRect.left - containerRect.left);
		} else if (elementRect.right > containerRect.right) {
			// Element is to the right of viewport
			scrollLeft = currentScrollLeft + (elementRect.right - containerRect.right);
		}
	}

	// Perform the scroll
	if (isWindow) {
		window.scrollTo({
			top: scrollTop,
			left: scrollLeft,
			behavior
		});
	} else {
		container.scrollTo({
			top: scrollTop,
			left: scrollLeft,
			behavior
		});
	}
}

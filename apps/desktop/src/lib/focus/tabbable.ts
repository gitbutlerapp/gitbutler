interface FocusOptions {
	/** Whether to wrap to the first element when reaching the end */
	wrap?: boolean;
	/** Container element to limit the search scope */
	container?: HTMLElement;
	/** Additional custom selector for focusable elements */
	customSelector?: string;
}

export function focusNextTabIndex(
	currentElement: HTMLElement,
	options: FocusOptions = {}
): boolean {
	const { wrap = true, container = document.documentElement, customSelector = '' } = options;

	// Build selector array
	const focusableSelectors: string[] = [
		'a[href]',
		'button:not([disabled])',
		'input:not([disabled]):not([type="hidden"])',
		'select:not([disabled])',
		'textarea:not([disabled])',
		'[tabindex]:not([tabindex="-1"])',
		'[contenteditable="true"]'
	];

	if (customSelector) {
		focusableSelectors.push(customSelector);
	}

	const focusableElements: NodeListOf<Element> = container.querySelectorAll(
		focusableSelectors.join(',')
	);

	const focusableArray: Element[] = Array.from(focusableElements)
		.filter((el): el is HTMLElement => {
			if (!(el instanceof HTMLElement)) return false;

			return (
				el.offsetWidth > 0 &&
				el.offsetHeight > 0 &&
				!el.hidden &&
				getComputedStyle(el).visibility !== 'hidden'
			);
		})
		.sort((a: HTMLElement, b: HTMLElement): number => {
			const aIndex: number = parseInt(a.getAttribute('tabindex') || '0') || 0;
			const bIndex: number = parseInt(b.getAttribute('tabindex') || '0') || 0;

			if (aIndex > 0 && bIndex === 0) return -1;
			if (aIndex === 0 && bIndex > 0) return 1;
			if (aIndex > 0 && bIndex > 0) return aIndex - bIndex;

			const position: number = a.compareDocumentPosition(b);
			return position & Node.DOCUMENT_POSITION_FOLLOWING ? -1 : 1;
		});

	const indexOf = document.activeElement
		? focusableArray.indexOf(document.activeElement)
		: undefined;
	let nextIndex = indexOf === undefined ? 0 : indexOf + 1;

	// Handle wrap behavior
	if (nextIndex >= focusableArray.length) {
		if (!wrap) return false;
		nextIndex = 0;
	}

	const nextElement: Element | undefined = focusableArray[nextIndex];
	if (nextElement instanceof HTMLElement) {
		nextElement.focus();
		return true;
	}

	return false;
}

let trapFocusList: HTMLElement[] = [];

function getFocusableElements(node: HTMLElement): HTMLElement[] {
	return Array.from(
		node.querySelectorAll<HTMLElement>(
			'a, button, input, textarea, select, details,[tabindex]:not([tabindex="-1"]):not([tabindex="0"])'
		)
	);
}

if (typeof window !== 'undefined') {
	function isNext(event: KeyboardEvent): boolean {
		return event.key === 'Tab' && !event.shiftKey;
	}
	function isPrevious(event: KeyboardEvent): boolean {
		return event.key === 'Tab' && event.shiftKey;
	}
	function trapFocusListener(event: KeyboardEvent) {
		if (event.target === window) {
			return;
		}

		const eventTarget = event.target as Node;

		const parentNode = trapFocusList.find((node) => node.contains(eventTarget));
		if (!parentNode) {
			return;
		}

		// List inspired by https://github.com/nico3333fr/van11y-accessible-modal-tooltip-aria/blob/master/src/van11y-accessible-modal-tooltip-aria.es6.js#L47C17-L47C17
		const focusable = getFocusableElements(parentNode);
		const first = focusable[0];
		const last = focusable[focusable.length - 1];

		if (isNext(event) && event.target === last) {
			event.preventDefault();
			first.focus();
		} else if (isPrevious(event) && event.target === first) {
			event.preventDefault();
			last.focus();
		}
	}

	document.addEventListener('keydown', trapFocusListener);
}

export function focusTrap(node: HTMLElement, focusOnFirst = true) {
	// focus on the first focusable element
	if (focusOnFirst) {
		const focusable = getFocusableElements(node);
		if (focusable.length) {
			focusable[0].focus();
		}
	}

	trapFocusList.push(node);
	return {
		destroy() {
			trapFocusList = trapFocusList.filter((element) => element !== node);
		}
	};
}

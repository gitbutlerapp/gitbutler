let trapFocusList: HTMLElement[] = [];

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
		const focusable: NodeListOf<HTMLElement> = parentNode.querySelectorAll(
			'a[href], area[href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), button:not([disabled]), iframe, object, embed, *[tabindex], *[contenteditable]'
		);
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

export function focusTrap(node: HTMLElement) {
	trapFocusList.push(node);
	return {
		destroy() {
			trapFocusList = trapFocusList.filter((element) => element !== node);
		}
	};
}

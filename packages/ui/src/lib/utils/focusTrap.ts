let trapFocusList: HTMLElement[] = [];

function getFocusableElements(node: HTMLElement): HTMLElement[] {
	return Array.from(
		node.querySelectorAll<HTMLElement>(
			'a, button, input, textarea, select, details, [tabindex]:not([tabindex="-1"])'
		)
	).filter((element) => isFocusable(element));
}

// Helper function to determine if an element is focusable
function isFocusable(element: HTMLElement): boolean {
	const style = window.getComputedStyle(element);

	const isVisible = style.display !== 'none' && style.visibility !== 'hidden';
	const isEnabled = !element.hasAttribute('disabled');

	return isVisible && isEnabled;
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

export type focusParams = { focusOnElement?: HTMLElement; focusOnFirst?: boolean };

export function focusTrap(node: HTMLElement, params: focusParams = { focusOnFirst: true }) {
	// focus on the first focusable element
	if (params.focusOnFirst && !params.focusOnElement) {
		const focusable = getFocusableElements(node);

		if (focusable.length) {
			focusable[0].focus();
			window.setTimeout(() => {
				focusable[0].focus();
			}, 0);
		}
	}

	// focus on the specified element
	if (params.focusOnElement) {
		console.log('focusOnElement', params.focusOnElement);
		params.focusOnElement.focus();
	}

	trapFocusList.push(node);

	return {
		destroy() {
			trapFocusList = trapFocusList.filter((element) => element !== node);
		}
	};
}

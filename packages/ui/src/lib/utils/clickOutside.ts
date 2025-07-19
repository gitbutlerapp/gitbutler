export type ClickOpts = {
	excludeElement?: Element | Element[];
	handler: (event: MouseEvent) => void;
};

function elementShouldBeExcluded(
	element: Element | Element[] | undefined,
	target: HTMLElement
): boolean {
	if (!element) return false;
	if (Array.isArray(element)) {
		return element.some((el) => el.contains(target));
	}
	return element.contains(target);
}

export function clickOutside(node: HTMLElement, params: ClickOpts) {
	function onClick(event: MouseEvent) {
		if (
			node &&
			!node.contains(event.target as HTMLElement) &&
			!elementShouldBeExcluded(params.excludeElement, event.target as HTMLElement)
		) {
			params.handler(event);
		}
	}
	document.addEventListener('pointerdown', onClick, true);
	document.addEventListener('contextmenu', onClick, true);
	return {
		destroy() {
			document.removeEventListener('pointerdown', onClick, true);
			document.removeEventListener('contextmenu', onClick, true);
		}
	};
}

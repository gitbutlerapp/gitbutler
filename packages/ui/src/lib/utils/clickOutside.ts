export type ClickOpts = { excludeElement?: Element; handler: (event: MouseEvent) => void };

export function clickOutside(node: HTMLElement, params: ClickOpts) {
	function onClick(event: MouseEvent) {
		if (
			node &&
			!node.contains(event.target as HTMLElement) &&
			!params.excludeElement?.contains(event.target as HTMLElement)
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

export function clickOutside(node: HTMLElement, handler: () => void): { destroy: () => void } {
	function onClick(event: MouseEvent) {
		if (node && !node.contains(event.target as HTMLElement)) {
			handler();
		}
	}

	document.addEventListener('click', onClick, true);
	return {
		destroy() {
			document.removeEventListener('click', onClick, true);
		}
	};
}

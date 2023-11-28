export function clickOutside(
	node: HTMLElement,
	params: { trigger?: HTMLElement; handler: () => void }
): { destroy: () => void } {
	console.log(params);
	function onClick(event: MouseEvent) {
		console.log(params.trigger?.contains(event.target as HTMLElement));
		if (
			node &&
			!node.contains(event.target as HTMLElement) &&
			!params.trigger?.contains(event.target as HTMLElement)
		) {
			params.handler();
		}
	}

	document.addEventListener('click', onClick, true);
	return {
		destroy() {
			document.removeEventListener('click', onClick, true);
		}
	};
}

export function preventTransitionOnMount(node: HTMLElement | SVGElement) {
	// This function prevents transitions from running when the element is mounted.
	function runTimer() {
		node.classList.add('h-no-transition');
		setTimeout(() => {
			node.classList.remove('h-no-transition');
		}, 100);
	}

	runTimer();

	return {
		destroy() {
			node.classList.remove('h-no-transition');
		}
	};
}

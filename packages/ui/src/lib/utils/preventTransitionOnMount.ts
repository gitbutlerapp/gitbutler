export function preventTransitionOnMount(node: HTMLElement) {
	// cunstruct class styles

	// run timer to prevent transition on mount
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

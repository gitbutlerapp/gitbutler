/**
 * Svelte action for passive scrolling.
 */
export function passiveScroll(node: Element, handler: (e: Event) => void) {
	node.addEventListener("scroll", handler, { passive: true });

	return {
		destroy() {
			node.removeEventListener("scroll", handler);
		},
	};
}

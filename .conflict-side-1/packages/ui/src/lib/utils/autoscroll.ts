export function useAutoScroll(node: HTMLElement, params: { enabled?: boolean }) {
	let { enabled } = params;
	let isAtBottom = true;
	const threshold = 30; // pixels

	function handleScroll() {
		const atBottom = node.scrollHeight - node.scrollTop - node.clientHeight < threshold;
		isAtBottom = atBottom;
	}

	function maybeScroll() {
		if (!enabled) return;
		if (isAtBottom || node.scrollHeight <= node.clientHeight) {
			node.scrollTop = node.scrollHeight;
		}
	}

	node.addEventListener('scroll', handleScroll);

	const mutationObserver = new MutationObserver(maybeScroll);
	mutationObserver.observe(node, { childList: true, subtree: true });

	const resizeObserver = new ResizeObserver(maybeScroll);
	resizeObserver.observe(node);

	return {
		update(newParams: { enabled: boolean }) {
			enabled = newParams.enabled;
			maybeScroll();
		},
		destroy() {
			node.removeEventListener('scroll', handleScroll);
			mutationObserver.disconnect();
			resizeObserver.disconnect();
		}
	};
}

export function portal(node: HTMLElement, to: string | Element) {
	const target = typeof to === 'string' ? document.querySelector(to) : to;
	if (target) {
		target.appendChild(node);
	}
	return {
		destroy() {
			if (node.isConnected) node.remove();
		}
	};
}

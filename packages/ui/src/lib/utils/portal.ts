export function portal(node: HTMLElement, to: string) {
	const target = document.querySelector(to);
	if (target) {
		target.appendChild(node);
	}
	return {
		destroy() {
			if (node.isConnected) node.remove();
		}
	};
}

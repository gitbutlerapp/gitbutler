export function accordion(node: HTMLDivElement, isOpen: boolean) {
	node.style.height = isOpen ? 'auto' : '0';
	node.style.overflow = 'hidden';
	return {
		update(isOpenInner: boolean) {
			node.style.display = isOpenInner ? 'initial' : 'none';
		}
	};
}

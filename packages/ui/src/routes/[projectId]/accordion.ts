const COLLAPSED_CLASS = 'collapsed';

export function accordion(node: HTMLDivElement, isOpen: boolean) {
	if (!isOpen) {
		node.classList.add(COLLAPSED_CLASS);
	}
	node.style.overflow = 'hidden';
	return {
		update(isOpenInner: boolean) {
			node.style.display = isOpenInner ? 'initial' : 'none';
			isOpenInner ? node.classList.remove(COLLAPSED_CLASS) : node.classList.add(COLLAPSED_CLASS);
		}
	};
}

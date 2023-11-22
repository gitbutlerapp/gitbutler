const COLLAPSED_CLASS = 'collapsed';

export function accordion(node: HTMLDivElement, isOpen: boolean) {
	if (!isOpen) {
		node.classList.add(COLLAPSED_CLASS);
	}
	return {
		update(isOpenInner: boolean) {
			isOpenInner ? node.classList.remove(COLLAPSED_CLASS) : node.classList.add(COLLAPSED_CLASS);
		}
	};
}

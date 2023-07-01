export function accordion(node: HTMLDivElement, isOpen: boolean) {
	const initialHeight = node.offsetHeight;
	node.style.height = isOpen ? 'auto' : '0';
	node.style.overflow = 'hidden';
	return {
		update(isOpenInner: boolean) {
			const animation = node.animate(
				[
					{
						height: initialHeight + 'px',
						overflow: 'visible'
					},
					{
						height: 0,
						overflow: 'hidden'
					}
				],
				{ duration: 100, fill: 'both' }
			);
			animation.pause();
			if (!isOpenInner) {
				animation.play();
			} else {
				animation.reverse();
				animation.play();
			}
		}
	};
}

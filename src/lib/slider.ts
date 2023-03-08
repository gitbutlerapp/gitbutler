export default (node: HTMLElement) => {
	const onDown = getOnDown(node);

	node.addEventListener('touchstart', onDown);
	node.addEventListener('mousedown', onDown);
	return {
		destroy() {
			node.removeEventListener('touchstart', onDown);
			node.removeEventListener('mousedown', onDown);
		}
	};
};

const getOnDown = (node: HTMLElement) => {
	const onMove = getOnMove(node);

	return (e: Event) => {
		e.preventDefault();
		node.dispatchEvent(new CustomEvent('dragstart'));

		const moveevent = 'touches' in e ? 'touchmove' : 'mousemove';
		const upevent = 'touches' in e ? 'touchend' : 'mouseup';

		const onUp = (e: Event) => {
			e.stopPropagation();

			document.removeEventListener(moveevent, onMove);
			document.removeEventListener(upevent, onUp);

			node.dispatchEvent(new CustomEvent('dragend'));
		};

		document.addEventListener(moveevent, onMove);
		document.addEventListener(upevent, onUp);
	};
};

const getOnMove = (node: HTMLElement) => {
	const track = node.parentNode as HTMLElement;

	return (e: TouchEvent | MouseEvent) => {
		const { left, width } = track.getBoundingClientRect();
		const clickOffset = 'touches' in e ? e.touches[0].clientX : e.clientX;
		const clickPos = Math.min(Math.max((clickOffset - left) / width, 0), 1) || 0;
		node.dispatchEvent(new CustomEvent('drag', { detail: clickPos }));
	};
};

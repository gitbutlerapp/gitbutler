import { intersectionObserver } from '$lib/utils/intersectionObserver';

export function stickyHeader(
	node: HTMLElement,
	options?: {
		align?: 'top' | 'bottom';
		onStick?: (flag: boolean) => void;
		unstyled?: boolean;
		disabled?: boolean;
	} | null
) {
	const { align = 'top', unstyled = false, onStick, disabled = false } = options || {};

	if (disabled) return;

	node.classList.add('h-sticky-header');
	node.classList.add(`h-sticky-header_${align}`);

	const stickyClass = `h-sticky-header_sticked-${align}`;

	intersectionObserver(node, {
		callback: (entry) => {
			if (entry?.isIntersecting) {
				if (!unstyled) node.classList.toggle(stickyClass, false);
				onStick?.(false);
			} else {
				if (!unstyled) node.classList.toggle(stickyClass, true);
				onStick?.(true);
			}
		},
		options: {
			root: null,
			rootMargin: `-1px`,
			threshold: 1
		}
	});
}

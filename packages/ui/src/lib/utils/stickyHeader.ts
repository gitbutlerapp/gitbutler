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

	// Base sticky positioning
	node.style.position = 'sticky';
	node.style.zIndex = 'var(--z-lifted)';

	const BORDER_WIDTH = '0.063rem'; // 1px in rem

	if (align === 'top') {
		node.style.top = `-${BORDER_WIDTH}`;
	} else {
		node.style.bottom = `-${BORDER_WIDTH}`;
	}

	function applyStickyStyles() {
		if (unstyled) return;

		node.style.borderBottom = `${BORDER_WIDTH} solid transparent`;

		if (align === 'top') {
			node.style.borderBottom = `${BORDER_WIDTH} solid var(--clr-border-2)`;
		} else {
			node.style.borderTop = `${BORDER_WIDTH} solid var(--clr-border-2)`;
		}
	}

	function removeStickyStyles() {
		if (unstyled) return;

		if (align === 'top') {
			node.style.removeProperty('border-bottom');
		} else {
			node.style.removeProperty('border-top');
		}
	}

	const cleanup = intersectionObserver(node, {
		callback: (entry) => {
			const isStuck = !entry?.isIntersecting;
			if (isStuck) {
				applyStickyStyles();
			} else {
				removeStickyStyles();
			}
			onStick?.(isStuck);
		},
		options: {
			root: null,
			rootMargin: '-1px',
			threshold: 1
		}
	});

	return {
		destroy() {
			cleanup?.destroy();
			removeStickyStyles();
		}
	};
}

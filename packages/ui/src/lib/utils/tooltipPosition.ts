import { onMount } from 'svelte';

function isNoSpaceOnRight(targetEl: HTMLElement, tooltipEl: HTMLElement) {
	if (!targetEl || !tooltipEl) return false;

	const tooltipRect = tooltipEl.getBoundingClientRect();
	const targetChild = targetEl.children[0];
	const targetRect = targetChild.getBoundingClientRect();

	return targetRect.left + tooltipRect.width / 2 > window.innerWidth;
}

function isNoSpaceOnLeft(targetEl: HTMLElement, tooltipEl: HTMLElement) {
	if (!targetEl || !tooltipEl) return false;

	const tooltipRect = tooltipEl.getBoundingClientRect();
	const targetChild = targetEl.children[0];
	const targetRect = targetChild.getBoundingClientRect();

	return targetRect.left - tooltipRect.width / 2 < 0;
}

// Calculate position based on alignment
function calculateHorizontalAlignment(
	targetRect: DOMRect,
	tooltipRect: DOMRect,
	align: string | undefined
) {
	switch (align) {
		case 'start':
			return { left: targetRect.left + window.scrollX, transformOriginLeft: 'left' };
		case 'end':
			return {
				left: targetRect.right - tooltipRect.width + window.scrollX,
				transformOriginLeft: 'right'
			};
		case 'center':
		default:
			return {
				left: targetRect.left + targetRect.width / 2 - tooltipRect.width / 2 + window.scrollX,
				transformOriginLeft: 'center'
			};
	}
}

// Auto-detect horizontal alignment if not specified
function autoDetectHorizontalAlignment(
	targetEl: HTMLElement,
	tooltipEl: HTMLElement,
	targetRect: DOMRect,
	tooltipRect: DOMRect
) {
	if (isNoSpaceOnLeft(targetEl, tooltipEl)) {
		return { left: targetRect.left + window.scrollX, transformOriginLeft: 'left' };
	}
	if (isNoSpaceOnRight(targetEl, tooltipEl)) {
		return {
			left: targetRect.right - tooltipRect.width + window.scrollX,
			transformOriginLeft: 'right'
		};
	}
	return {
		left: targetRect.left + targetRect.width / 2 - tooltipRect.width / 2 + window.scrollX,
		transformOriginLeft: 'center'
	};
}

// Calculate vertical position based on the 'top' or 'bottom' position
function calculateVerticalPosition(
	targetRect: DOMRect,
	tooltipRect: DOMRect,
	position: string | undefined,
	gap: number
) {
	const scrollY = window.scrollY;

	if (position === 'top') {
		return {
			top: targetRect.top - tooltipRect.height + scrollY - gap,
			transformOriginTop: 'bottom'
		};
	}
	// Default to 'bottom' position if not specified
	return { top: targetRect.bottom + scrollY + gap, transformOriginTop: 'top' };
}

export function setPosition(
	tooltipNode: HTMLElement | undefined,
	props: {
		targetEl: HTMLElement | undefined;
		position?: 'top' | 'bottom';
		align?: 'start' | 'center' | 'end';
		gap?: number;
	}
) {
	onMount(() => {
		const { targetEl, position, align, gap = 4 } = props;

		if (!targetEl || !tooltipNode) return;

		const tooltipRect = tooltipNode.getBoundingClientRect();
		const targetChild = targetEl.children[0];
		const targetRect = targetChild.getBoundingClientRect();

		// Determine vertical position
		const { top, transformOriginTop } = calculateVerticalPosition(
			targetRect,
			tooltipRect,
			position,
			gap
		);

		// Determine horizontal alignment (either specified or auto-detected)
		const { left, transformOriginLeft } = align
			? calculateHorizontalAlignment(targetRect, tooltipRect, align)
			: autoDetectHorizontalAlignment(targetEl, tooltipNode, targetRect, tooltipRect);

		// Apply calculated styles
		tooltipNode.style.top = `${top}px`;
		tooltipNode.style.left = `${left}px`;
		tooltipNode.style.transformOrigin = `${transformOriginTop} ${transformOriginLeft}`;
	});
}

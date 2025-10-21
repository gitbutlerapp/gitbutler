interface TooltipPosition {
	top: number;
	left: number;
	transformOrigin: string;
}

interface PositionProps {
	targetEl?: HTMLElement;
	position?: 'top' | 'bottom';
	align?: 'start' | 'center' | 'end';
	verticalOffset?: number;
	overrideYScroll?: number;
}

interface MeasurementCache {
	targetRect: DOMRect;
	tooltipRect: DOMRect;
	windowWidth: number;
	windowHeight: number;
	scrollX: number;
	scrollY: number;
}

function getMeasurements(
	targetEl: HTMLElement,
	tooltipEl: HTMLElement,
	overrideYScroll?: number
): MeasurementCache {
	const targetChild = targetEl.children[0] as HTMLElement;

	return {
		targetRect: targetChild.getBoundingClientRect(),
		tooltipRect: tooltipEl.getBoundingClientRect(),
		windowWidth: window.innerWidth,
		windowHeight: window.innerHeight,
		scrollX: window.scrollX,
		scrollY: overrideYScroll ?? window.scrollY
	};
}

function hasSpaceOnRight(measurements: MeasurementCache): boolean {
	const { targetRect, tooltipRect, windowWidth } = measurements;
	return targetRect.left + targetRect.width / 2 + tooltipRect.width / 2 <= windowWidth;
}

function hasSpaceOnLeft(measurements: MeasurementCache): boolean {
	const { targetRect, tooltipRect } = measurements;
	return targetRect.left + targetRect.width / 2 - tooltipRect.width / 2 >= 0;
}

function hasSpaceBelow(measurements: MeasurementCache, offset: number): boolean {
	const { targetRect, tooltipRect, windowHeight } = measurements;
	return targetRect.bottom + tooltipRect.height + offset <= windowHeight;
}

function calculateHorizontalAlignment(
	measurements: MeasurementCache,
	align: 'start' | 'center' | 'end'
): { left: number; transformOriginX: string } {
	const { targetRect, tooltipRect, scrollX } = measurements;

	switch (align) {
		case 'start':
			return {
				left: targetRect.left + scrollX,
				transformOriginX: 'left'
			};
		case 'end':
			return {
				left: targetRect.right - tooltipRect.width + scrollX,
				transformOriginX: 'right'
			};
		default:
			return {
				left: targetRect.left + targetRect.width / 2 - tooltipRect.width / 2 + scrollX,
				transformOriginX: 'center'
			};
	}
}

function autoDetectHorizontalAlignment(measurements: MeasurementCache): {
	left: number;
	transformOriginX: string;
} {
	const { targetRect, tooltipRect, scrollX } = measurements;

	if (hasSpaceOnLeft(measurements) && hasSpaceOnRight(measurements)) {
		return {
			left: targetRect.left + targetRect.width / 2 - tooltipRect.width / 2 + scrollX,
			transformOriginX: 'center'
		};
	}

	if (!hasSpaceOnLeft(measurements)) {
		return {
			left: targetRect.left + scrollX,
			transformOriginX: 'left'
		};
	}

	return {
		left: targetRect.right - tooltipRect.width + scrollX,
		transformOriginX: 'right'
	};
}

function calculateVerticalPosition(
	measurements: MeasurementCache,
	position: 'top' | 'bottom' | undefined,
	offset: number
): { top: number; transformOriginY: string } {
	const { targetRect, scrollY } = measurements;

	if (!position) {
		position = hasSpaceBelow(measurements, offset) ? 'bottom' : 'top';
	}

	if (position === 'top') {
		return {
			top: targetRect.top - measurements.tooltipRect.height - offset + scrollY,
			transformOriginY: 'bottom'
		};
	}

	return {
		top: targetRect.bottom + offset + scrollY,
		transformOriginY: 'top'
	};
}

export function calculateTooltipPosition(
	targetEl: HTMLElement,
	tooltipEl: HTMLElement,
	props: PositionProps
): TooltipPosition {
	const { position, align, verticalOffset = 4, overrideYScroll } = props;

	const measurements = getMeasurements(targetEl, tooltipEl, overrideYScroll);
	const { top, transformOriginY } = calculateVerticalPosition(
		measurements,
		position,
		verticalOffset
	);
	const { left, transformOriginX } = align
		? calculateHorizontalAlignment(measurements, align)
		: autoDetectHorizontalAlignment(measurements);

	return {
		top,
		left,
		transformOrigin: `${transformOriginX} ${transformOriginY}`
	};
}

export function tooltip(tooltipNode: HTMLElement, props: PositionProps) {
	function updatePosition() {
		if (!props.targetEl) {
			return;
		}
		const position = calculateTooltipPosition(props.targetEl, tooltipNode, props);

		tooltipNode.style.top = `${position.top}px`;
		tooltipNode.style.left = `${position.left}px`;
		tooltipNode.style.transformOrigin = position.transformOrigin;
	}

	updatePosition();

	return {
		update(newProps: PositionProps) {
			Object.assign(props, newProps);
			updatePosition();
		}
	};
}

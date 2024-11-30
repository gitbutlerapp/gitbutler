<script lang="ts">
	import { pxToRem } from '@gitbutler/ui/utils/pxToRem';
	import { createEventDispatcher } from 'svelte';
	import { createBubbler, stopPropagation } from 'svelte/legacy';

	const bubble = createBubbler();
	import { pxToRem } from '@gitbutler/ui/utils/pxToRem';

	interface Props {
		// The element that is being resized
		viewport: HTMLElement;
		// Sets direction of resizing for viewport
		direction: 'left' | 'right' | 'up' | 'down';
		// Sets the color of the line
		defaultLineColor?: string;
		defaultLineThickness?: number;
		hoverLineThickness?: number;
		// Needed when overflow is hidden
		sticky?: boolean;
		// Custom z-index in case of overlapping with other elements
		zIndex?: string;
		//
		minWidth?: number;
		minHeight?: number;

		// Actions
		onHeight?: (height: number) => void;
		onWidth?: (width: number) => void;
		onResizing?: (isResizing: boolean) => void;
		onOverflow?: (value: number) => void;
		onHover?: (isHovering: boolean) => void;
		onDblClick?: () => void;
	}

	let {
		viewport,
		direction,
		defaultLineColor = 'none',
		defaultLineThickness = 1,
		hoverLineThickness = 2,
		sticky = false,
		zIndex = 'var(--z-lifted)',
		minWidth = 0,
		minHeight = 0,

		onHeight,
		onWidth,
		onResizing,
		onOverflow,
		onHover,
		onDblClick
	}: Props = $props();

	const orientation = $derived(['left', 'right'].includes(direction) ? 'horizontal' : 'vertical');

	let initial = 0;
	let dragging = $state(false);

	function onMouseDown(e: MouseEvent) {
		e.stopPropagation();
		e.preventDefault();
		document.addEventListener('mouseup', onMouseUp);
		document.addEventListener('mousemove', onMouseMove);

		if (direction === 'right') initial = e.clientX - viewport.clientWidth;
		if (direction === 'left') initial = window.innerWidth - e.clientX - viewport.clientWidth;
		if (direction === 'down') initial = e.clientY - viewport.clientHeight;
		if (direction === 'up') initial = window.innerHeight - e.clientY - viewport.clientHeight;

		onResizing?.(true);
	}

	function onOverflowValue(currentValue: number, minVal: number) {
		if (currentValue < minVal) {
			onOverflow?.(minVal - currentValue);
		}
	}

	function onMouseMove(e: MouseEvent) {
		dragging = true;
		if (direction === 'down') {
			let height = e.clientY - initial;
			onHeight?.(Math.max(height, minHeight));

			onOverflowValue(height, minHeight);
		}
		if (direction === 'up') {
			let height = document.body.scrollHeight - e.clientY - initial;
			onHeight?.(Math.max(height, minHeight));

			onOverflowValue(height, minHeight);
		}
		if (direction === 'right') {
			let width = e.clientX - initial + 2;
			onWidth?.(Math.max(width, minWidth));

			onOverflowValue(width, minWidth);
		}
		if (direction === 'left') {
			let width = document.body.scrollWidth - e.clientX - initial;
			onWidth?.(Math.max(width, minWidth));

			onOverflowValue(width, minWidth);
		}
	}

	function onMouseUp() {
		dragging = false;
		document.removeEventListener('mouseup', onMouseUp);
		document.removeEventListener('mousemove', onMouseMove);
		onResizing?.(false);
	}

	function isHovered(isHovered: boolean) {
		onHover?.(isHovered);
	}
</script>

<div
	data-remove-from-draggable
	onmousedown={onMouseDown}
	ondblclick={onDblClick}
	onmouseenter={() => isHovered(true)}
	onmouseleave={() => isHovered(false)}
	tabindex="0"
	role="slider"
	aria-valuenow={viewport?.clientHeight}
	class="resizer"
	class:dragging
	class:vertical={orientation === 'vertical'}
	class:horizontal={orientation === 'horizontal'}
	class:up={direction === 'up'}
	class:down={direction === 'down'}
	class:left={direction === 'left'}
	class:right={direction === 'right'}
	class:sticky
	style:z-index={zIndex}
>
	<div
		class="resizer-line"
		style="--resizer-default-line-color: {defaultLineColor}; --resizer-default-line-thickness: {pxToRem(
			defaultLineThickness
		)}; --resizer-hover-line-thickness: {pxToRem(hoverLineThickness)}"
	></div>
</div>

<style lang="postcss">
	.resizer {
		--resizer-frame-thickness: 4px;
		--resizer-default-line-thickness: 2px;
		--resizer-hover-line-thickness: 8px;
		--resizer-default-line-color: none;
		position: absolute;

		&:hover,
		&:focus,
		&.dragging {
			outline: none;

			& .resizer-line {
				transition-delay: 0.1s;
				background-color: var(--resizer-color);
			}

			&:not(.vertical) {
				& .resizer-line {
					width: var(--resizer-hover-line-thickness);
				}
			}

			&:not(.horizontal) {
				& .resizer-line {
					height: var(--resizer-hover-line-thickness);
				}
			}
		}
	}
	.resizer-line {
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		bottom: 0;
		background-color: var(--resizer-default-line-color);
		pointer-events: none;
		transition:
			background-color 0.1s ease-out,
			width 0.1s ease-out,
			height 0.1s ease-out;
	}

	.horizontal {
		width: 4px;
		height: 100%;
		cursor: col-resize;
		top: 0;

		& .resizer-line {
			width: var(--resizer-default-line-thickness);
		}
	}
	.vertical {
		height: 4px;
		width: 100%;
		cursor: row-resize;
		left: 0;

		& .resizer-line {
			height: var(--resizer-default-line-thickness);
		}
	}

	.right {
		right: 0;

		& .resizer-line {
			left: auto;
		}
	}
	.left {
		left: 0;

		& .resizer-line {
			right: auto;
		}
	}
	.up {
		top: 0;

		& .resizer-line {
			bottom: auto;
		}
	}
	.down {
		bottom: 0;

		& .resizer-line {
			top: auto;
		}
	}

	.sticky {
		position: sticky;
		top: 0;
	}
</style>

<script lang="ts">
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { getContextStoreBySymbol } from '@gitbutler/shared/context';

	interface Props {
		/** The element that is being resized */
		viewport: HTMLElement;
		/** Sets direction of resizing for viewport */
		direction: 'left' | 'right' | 'up' | 'down';
		/** Border radius for cases when the resizable element has rounded corners */
		borderRadius?: 's' | 'm' | 'ml' | 'l' | 'none';
		/** Custom z-index in case of overlapping with other elements */
		zIndex?: string;
		/** imitate border */
		imitateBorder?: boolean;
		imitateBorderColor?: string;

		/** Minimum width for the resizable element */
		minWidth?: number;
		maxWidth?: number;
		minHeight?: number;

		// Actions
		onHeight?: (height: number) => void;
		onWidth?: (width: number) => void;
		onResizing?: (isResizing: boolean) => void;
		onOverflow?: (value: number) => void;
		onHover?: (isHovering: boolean) => void;
		onDblClick?: () => void;
	}

	const {
		viewport,
		direction,
		zIndex = 'var(--z-lifted)',
		minWidth = 0,
		maxWidth = 40,
		minHeight = 0,
		borderRadius = 'none',
		imitateBorder,
		imitateBorderColor = 'var(--clr-border-2)',

		onHeight,
		onWidth,
		onResizing,
		onOverflow,
		onHover,
		onDblClick
	}: Props = $props();

	const orientation = $derived(['left', 'right'].includes(direction) ? 'horizontal' : 'vertical');
	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);
	const base = $derived($userSettings.zoom * 16);

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
			let height = (e.clientY - initial) / base;
			onHeight?.(Math.max(height, minHeight));

			onOverflowValue(height, minHeight);
		}
		if (direction === 'up') {
			let height = (document.body.scrollHeight - e.clientY - initial) / base;
			onHeight?.(Math.max(height, minHeight));

			onOverflowValue(height, minHeight);
		}
		if (direction === 'right') {
			let width = (e.clientX - initial + 2) / base;
			onWidth?.(Math.min(Math.max(width, minWidth), maxWidth));

			onOverflowValue(width, minWidth);
		}
		if (direction === 'left') {
			let width = (document.body.scrollWidth - e.clientX - initial) / base;
			onWidth?.(Math.min(Math.max(width, minWidth), maxWidth));

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
	style:z-index={zIndex}
	style:--resizer-border-radius="var(--radius-{borderRadius})"
	style:--border-imitation-color={imitateBorderColor}
>
	<div class="resizer-line"></div>

	{#if imitateBorder}
		<div class="border-imitation"></div>
	{/if}
</div>

<style lang="postcss">
	.resizer {
		--resizer-line-thickness: 0;
		--resizer-line-color: transparent;
		/* should be big for large radius */
		--resizer-line-frame: 20px;
		position: absolute;
		outline: none;
		/* background-color: rgba(255, 0, 0, 0.2); */

		&:hover,
		&:focus,
		&.dragging {
			--resizer-line-color: var(--resizer-color);
			--resizer-line-thickness: 0.15rem;

			& .resizer-line {
				transition-delay: 0.1s;
			}
		}
	}

	.resizer-line {
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		bottom: 0;
		pointer-events: none;
		transition: border 0.1s ease;
	}

	.horizontal {
		width: 8px;
		height: 100%;
		cursor: col-resize;
		top: 0;

		& .resizer-line {
			width: var(--resizer-line-frame);
		}

		& .border-imitation {
			width: 1px;
		}
	}

	.vertical {
		height: 4px;
		width: 100%;
		cursor: row-resize;
		left: 0;

		& .resizer-line {
			height: var(--resizer-line-frame);
		}

		& .border-imitation {
			height: 1px;
		}
	}

	.right {
		right: 0;

		& .resizer-line {
			left: auto;
			border-right: var(--resizer-line-thickness) solid var(--resizer-line-color);
			border-top-right-radius: var(--resizer-border-radius);
			border-bottom-right-radius: var(--resizer-border-radius);
		}

		& .border-imitation {
			left: auto;
		}
	}
	.left {
		left: 0;

		& .resizer-line {
			right: auto;
			border-left: var(--resizer-line-thickness) solid var(--resizer-line-color);
			border-top-left-radius: var(--resizer-border-radius);
			border-bottom-left-radius: var(--resizer-border-radius);
		}

		& .border-imitation {
			right: auto;
		}
	}
	.up {
		top: 0;

		& .resizer-line {
			bottom: auto;
			border-top: var(--resizer-line-thickness) solid var(--resizer-line-color);
			border-top-left-radius: var(--resizer-border-radius);
			border-top-right-radius: var(--resizer-border-radius);
		}
	}
	.down {
		bottom: 0;

		& .resizer-line {
			top: auto;
			border-bottom: var(--resizer-line-thickness) solid var(--resizer-line-color);
			border-bottom-left-radius: var(--resizer-border-radius);
			border-bottom-right-radius: var(--resizer-border-radius);
		}
	}

	.border-imitation {
		position: absolute;
		width: 100%;
		height: 100%;
		top: 0;
		left: 0;
		right: 0;
		bottom: 0;
		background-color: var(--border-imitation-color);
		z-index: -1;
	}
</style>

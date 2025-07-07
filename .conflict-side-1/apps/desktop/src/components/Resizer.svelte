<script lang="ts">
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { ResizeSync } from '$lib/utils/resizeSync';
	import { getContext, getContextStoreBySymbol } from '@gitbutler/shared/context';
	import { persistWithExpiration } from '@gitbutler/shared/persisted';
	import { writable } from 'svelte/store';

	interface Props {
		/** Default value */
		defaultValue?: number;
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
		borderColor?: string;
		/** Other resizers with the same name will receive same updates. */
		syncName?: string;
		/** Name under which the latest width is stored. */
		persistId?: string;
		/** Minimum width for the resizable element */
		minWidth?: number;
		maxWidth?: number;
		maxHeight?: number;
		minHeight?: number;
		/** Enabled, but does not set the width/height on the dom element */
		passive?: boolean;
		/** Doubles or halves the width on double click */
		clientHeight?: number;

		// Actions
		onHeight?: (height: number) => void;
		onWidth?: (width: number) => void;
		onResizing?: (isResizing: boolean) => void;
		onOverflow?: (value: number) => void;
		onHover?: (isHovering: boolean) => void;
		onDblClick?: () => void;
	}

	let {
		defaultValue,
		viewport,
		direction,
		zIndex = 'var(--z-floating)',
		minWidth = 0,
		maxWidth = 120,
		minHeight = 0,
		maxHeight = 120,
		borderRadius = 'none',
		imitateBorder,
		borderColor = 'var(--clr-border-2)',
		syncName,
		persistId,
		passive,
		clientHeight = $bindable(),
		onResizing,
		onOverflow,
		onHover,
		onDblClick,
		onWidth
	}: Props = $props();

	const orientation = $derived(['left', 'right'].includes(direction) ? 'horizontal' : 'vertical');
	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);
	const resizeSync = getContext(ResizeSync);
	const base = $derived($userSettings.zoom * 16);

	let value = $derived(
		persistId
			? persistWithExpiration(defaultValue, persistId, 1440)
			: writable<number | undefined>()
	);

	const resizerId = Symbol();

	let initial = 0;
	let dragging = $state(false);
	let resizerDiv = $state<HTMLDivElement>();

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

	function applyLimits(value: number) {
		let newValue: number | undefined;
		let overflow: number | undefined;
		if (direction === 'down') {
			newValue = Math.min(Math.max(value, minHeight), maxHeight);
			overflow = minHeight - value;
		}
		if (direction === 'up') {
			newValue = Math.min(Math.max(value, minHeight), maxHeight);
			overflow = minHeight - value;
		}
		if (direction === 'right') {
			newValue = Math.min(Math.max(value, minWidth), maxWidth);
			overflow = minWidth - value;
		}
		if (direction === 'left') {
			newValue = Math.min(Math.max(value, minWidth), maxWidth);
			overflow = minWidth - value;
		}

		return { newValue, overflow };
	}

	function onMouseMove(e: MouseEvent) {
		dragging = true;
		let limitedValue:
			| {
					newValue?: number;
					overflow?: number;
			  }
			| undefined;
		if (direction === 'down') {
			limitedValue = applyLimits((e.clientY - initial) / base);
		}
		if (direction === 'up') {
			limitedValue = applyLimits((document.body.scrollHeight - e.clientY - initial) / base);
		}
		if (direction === 'right') {
			limitedValue = applyLimits((e.clientX - initial + 2) / base);
		}
		if (direction === 'left') {
			limitedValue = applyLimits((document.body.scrollWidth - e.clientX - initial) / base);
		}
		if (!limitedValue) {
			return;
		}
		const { newValue, overflow } = limitedValue;
		if (newValue && !passive) {
			value.set(newValue);
			updateDom(newValue);
			onWidth?.(newValue);
		}
		if (overflow) {
			onOverflow?.(overflow);
		}
		if (e.shiftKey && syncName && newValue !== undefined && !passive) {
			resizeSync.emit(syncName, resizerId, newValue);
		}
	}

	function onMouseUp() {
		dragging = false;
		document.removeEventListener('mouseup', onMouseUp);
		document.removeEventListener('mousemove', onMouseMove);
		onResizing?.(false);
	}

	function updateDom(newValue?: number) {
		if (!viewport) {
			return;
		}
		if (passive) {
			viewport.style.width = '';
			viewport.style.height = '';
			return;
		}

		if (newValue === undefined) {
			newValue = defaultValue;
		} else {
			newValue = applyLimits(newValue).newValue;
		}

		if (newValue === undefined) {
			viewport.style.width = '';
			viewport.style.height = '';
		} else if (direction === 'left' || direction === 'right') {
			viewport.style.width = newValue + 'rem';
		} else if (direction === 'up' || direction === 'down') {
			viewport.style.height = newValue + 'rem';
		}
	}

	function isHovered(isHovered: boolean) {
		onHover?.(isHovered);
	}

	$effect(() => {
		if (syncName) {
			return resizeSync.subscribe({
				key: syncName,
				resizerId,
				callback: (newValue) => value.set(newValue)
			});
		}
	});

	$effect(() => {
		if ($value) {
			updateDom($value);
			onWidth?.($value);
		}
	});

	$effect(() => {
		if (maxWidth || minWidth || maxHeight || minHeight) {
			updateDom($value);
		}
	});

	$effect(() => {
		if (viewport) {
			clientHeight = viewport.clientHeight;
			const monitor = new ResizeObserver((e) => (clientHeight = e[0]?.target.clientHeight));
			monitor.observe(viewport);
			return () => monitor.disconnect();
		}
	});
</script>

<div
	bind:this={resizerDiv}
	data-remove-from-draggable
	onmousedown={onMouseDown}
	ondblclick={onDblClick}
	onmouseenter={() => isHovered(true)}
	onmouseleave={() => isHovered(false)}
	tabindex="0"
	role="slider"
	aria-valuenow={viewport?.clientHeight}
	class:imitate-border={imitateBorder}
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
	style:--border-imitation-color={borderColor}
>
	<div class="resizer-line"></div>
</div>

<style lang="postcss">
	.resizer {
		--resizer-line-thickness: 0;
		--resizer-line-color: transparent;
		/* should be big for large radius */
		--resizer-line-frame: var(--resizer-border-radius, 1px);
		position: absolute;
		outline: none;
		/* background-color: rgba(255, 0, 0, 0.2); */

		&.imitate-border {
			--resizer-line-color: var(--border-imitation-color);
			--resizer-line-thickness: 1px;
		}

		&:hover,
		&:focus,
		&.dragging {
			--resizer-line-color: var(--resizer-color);
			--resizer-line-thickness: 0.14rem;

			& .resizer-line {
				transition-delay: 0.1s;
			}
		}
	}

	.resizer-line {
		position: absolute;
		top: 0;
		right: 0;
		bottom: 0;
		left: 0;
		pointer-events: none;
		transition: border 0.1s ease;
	}

	.horizontal {
		top: 0;
		width: 4px;
		height: 100%;
		cursor: col-resize;

		& .resizer-line {
			width: var(--resizer-line-frame);
		}
	}

	.vertical {
		left: 0;
		width: 100%;
		height: 4px;
		cursor: row-resize;

		& .resizer-line {
			height: var(--resizer-line-frame);
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
	}
	.left {
		left: 0;

		& .resizer-line {
			right: auto;
			border-left: var(--resizer-line-thickness) solid var(--resizer-line-color);
			border-top-left-radius: var(--resizer-border-radius);
			border-bottom-left-radius: var(--resizer-border-radius);
		}
	}
	.up {
		top: 0;

		& .resizer-line {
			bottom: auto;
			border-top: var(--resizer-line-thickness) solid var(--resizer-line-color);
			border-top-right-radius: var(--resizer-border-radius);
			border-top-left-radius: var(--resizer-border-radius);
		}
	}
	.down {
		bottom: 0;

		& .resizer-line {
			top: auto;
			border-bottom: var(--resizer-line-thickness) solid var(--resizer-line-color);
			border-bottom-right-radius: var(--resizer-border-radius);
			border-bottom-left-radius: var(--resizer-border-radius);
		}
	}
</style>

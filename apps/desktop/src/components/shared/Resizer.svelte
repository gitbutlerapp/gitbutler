<script lang="ts">
	import { SETTINGS } from '$lib/settings/userSettings';
	import { RESIZE_SYNC } from '$lib/utils/resizeSync';
	import { inject } from '@gitbutler/shared/context';
	import { persistWithExpiration } from '@gitbutler/shared/persisted';
	import { mergeUnlisten } from '@gitbutler/ui/utils/mergeUnlisten';
	import { pxToRem, remToPx } from '@gitbutler/ui/utils/pxToRem';
	import { writable } from 'svelte/store';
	import type { ResizeGroup } from '$lib/utils/resizeGroup';

	interface Props {
		/** Default value */
		defaultValue: number | undefined;
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
		/** Whether the resizer is hidden */
		hidden?: boolean;
		/** Optional manager that can coordinate multiple resizers */
		resizeGroup?: ResizeGroup;
		/** Optional ordering of resizer for use with `resizeManager` */
		order?: number;
		/** Unset max height */
		unsetMaxHeight?: string;

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
		hidden,
		resizeGroup,
		order,
		unsetMaxHeight,
		onResizing,
		onOverflow,
		onHover,
		onDblClick,
		onWidth
	}: Props = $props();

	const orientation = $derived(['left', 'right'].includes(direction) ? 'horizontal' : 'vertical');
	const userSettings = inject(SETTINGS);
	const resizeSync = inject(RESIZE_SYNC);
	const zoom = $derived($userSettings.zoom);

	const value = $derived(
		persistId
			? persistWithExpiration(defaultValue, persistId, 1440)
			: writable<number | undefined>(defaultValue)
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
		let newValue: number;
		let overflow: number;
		switch (direction) {
			case 'down':
				newValue = Math.min(Math.max(value, minHeight), maxHeight);
				overflow = minHeight - value;
				break;
			case 'up':
				newValue = Math.min(Math.max(value, minHeight), maxHeight);
				overflow = minHeight - value;
				break;
			case 'right':
				newValue = Math.min(Math.max(value, minWidth), maxWidth);
				overflow = minWidth - value;
				break;
			case 'left':
				newValue = Math.min(Math.max(value, minWidth), maxWidth);
				overflow = minWidth - value;
				break;
		}

		return { newValue, overflow };
	}

	function onMouseMove(e: MouseEvent) {
		dragging = true;
		let offsetPx: number | undefined;
		switch (direction) {
			case 'down':
				offsetPx = e.clientY - initial;
				break;
			case 'up':
				offsetPx = document.body.scrollHeight - e.clientY - initial;
				break;
			case 'right':
				offsetPx = e.clientX - initial + 2;
				break;
			case 'left':
				offsetPx = document.body.scrollWidth - e.clientX - initial;
				break;
		}

		const offsetRem = pxToRem(offsetPx, zoom);

		// Presence of a resize group means we hand off the rest of the
		// handling of this event.
		if (resizeGroup) {
			const subtracted = resizeGroup.resize(resizerId, offsetRem);
			// The initial offset needs to be adjusted if an adjustement
			// means the whole resizer has moved.
			initial = initial - remToPx(subtracted, zoom);
			return;
		}

		const { newValue, overflow } = applyLimits(offsetRem);

		if (newValue && !passive && !hidden) {
			setValue(newValue);
		}
		if (overflow) {
			onOverflow?.(overflow);
		}
		if (e.shiftKey && syncName && newValue !== undefined && !passive && !hidden) {
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
		if (passive || hidden) {
			if (orientation === 'horizontal') {
				viewport.style.width = '';
				viewport.style.maxWidth = '';
				viewport.style.minWidth = '';
			} else {
				viewport.style.height = '';
				viewport.style.maxHeight = '';
				viewport.style.minHeight = '';
			}
			return;
		}

		if (newValue !== undefined) {
			newValue = applyLimits(newValue).newValue;
		}

		if (orientation === 'horizontal') {
			if (newValue === undefined) {
				viewport.style.width = '';
				viewport.style.maxWidth = maxWidth ? maxWidth + 'rem' : '';
				viewport.style.minWidth = minWidth ? minWidth + 'rem' : '';
			} else {
				viewport.style.width = newValue + 'rem';
				viewport.style.maxWidth = '';
				viewport.style.minWidth = '';
			}
		} else {
			if (newValue === undefined) {
				viewport.style.height = '';
				viewport.style.maxHeight = unsetMaxHeight || '';
				viewport.style.minHeight = minHeight ? minHeight + 'rem' : '';
			} else {
				viewport.style.height = newValue + 'rem';
				viewport.style.maxHeight = '';
				viewport.style.minHeight = '';
			}
		}
	}

	function isHovered(isHovered: boolean) {
		onHover?.(isHovered);
	}

	function getValue() {
		if ($value !== undefined) {
			return $value;
		}
		if (orientation === 'horizontal') {
			return pxToRem(viewport.clientWidth, zoom);
		}
		return pxToRem(viewport.clientHeight, zoom);
	}

	export function setValue(newValue?: number) {
		const currentValue = getValue();
		if (currentValue === newValue) {
			return;
		}
		value.set(newValue);
		updateDom(newValue);
		if (newValue !== undefined) {
			onWidth?.(newValue);
		}
	}

	$effect(() => {
		if (resizeGroup && order !== undefined) {
			// It's important we do not make use of maxValue in the resize
			// manager, and in this effect. It changes with the value of
			// neighbors and would make this effect trigger constantly.
			return resizeGroup?.register({
				resizerId,
				getValue,
				setValue,
				minValue: minHeight || minWidth,
				position: order
			});
		}
	});

	$effect(() => {
		if (syncName) {
			const unlistenFns = [];
			unlistenFns.push(
				resizeSync.subscribe({
					key: syncName,
					resizerId,
					callback: setValue
				})
			);
			return mergeUnlisten(...unlistenFns);
		}
	});

	$effect(() => {
		if (maxWidth || minWidth || maxHeight || minHeight) {
			updateDom($value);
			if ($value !== undefined) {
				onWidth?.($value);
			}
		}
	});
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	bind:this={resizerDiv}
	data-remove-from-draggable
	onmousedown={onMouseDown}
	ondblclick={() => {
		onDblClick?.();
		setValue(defaultValue);
	}}
	onmouseenter={() => isHovered(true)}
	onmouseleave={() => isHovered(false)}
	tabindex="0"
	class:hidden
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
		/* resizer-line-frame should be the same as border-radius */
		--resizer-line-frame: var(--resizer-border-radius, 1px);
		--resizer-cursor: default;
		position: absolute;
		outline: none;
		cursor: var(--resizer-cursor);

		&.imitate-border {
			--resizer-line-color: var(--border-imitation-color);
			--resizer-line-thickness: 1px;
		}

		&:not(.hidden) {
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

		&.horizontal {
			--resizer-cursor: col-resize;
			top: 0;
			width: 4px;
			height: 100%;

			& .resizer-line {
				width: var(--resizer-line-frame);
			}
		}

		&.vertical {
			--resizer-cursor: row-resize;
			left: 0;
			width: 100%;
			height: 4px;

			& .resizer-line {
				height: var(--resizer-line-frame);
			}
		}

		&.right {
			right: 0;

			& .resizer-line {
				left: auto;
				border-right: var(--resizer-line-thickness) solid var(--resizer-line-color);
				border-top-right-radius: var(--resizer-border-radius);
				border-bottom-right-radius: var(--resizer-border-radius);
			}
		}
		&.left {
			left: 0;

			& .resizer-line {
				right: auto;
				border-left: var(--resizer-line-thickness) solid var(--resizer-line-color);
				border-top-left-radius: var(--resizer-border-radius);
				border-bottom-left-radius: var(--resizer-border-radius);
			}
		}
		&.up {
			top: 0;

			& .resizer-line {
				bottom: auto;
				border-top: var(--resizer-line-thickness) solid var(--resizer-line-color);
				border-top-right-radius: var(--resizer-border-radius);
				border-top-left-radius: var(--resizer-border-radius);
			}
		}
		&.down {
			bottom: 0;

			& .resizer-line {
				top: auto;
				border-bottom: var(--resizer-line-thickness) solid var(--resizer-line-color);
				border-bottom-right-radius: var(--resizer-border-radius);
				border-bottom-left-radius: var(--resizer-border-radius);
			}
		}

		&.hidden {
			pointer-events: none;
			--resizer-cursor: default;
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
</style>

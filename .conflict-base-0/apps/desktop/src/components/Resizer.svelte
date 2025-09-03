<script lang="ts">
	import { SETTINGS } from '$lib/settings/userSettings';
	import { RESIZE_SYNC } from '$lib/utils/resizeSync';
	import { inject } from '@gitbutler/core/context';
	import { persistWithExpiration } from '@gitbutler/shared/persisted';
	import { mergeUnlisten } from '@gitbutler/ui/utils/mergeUnlisten';
	import { pxToRem, remToPx } from '@gitbutler/ui/utils/pxToRem';
	import { on } from 'svelte/events';
	import { writable } from 'svelte/store';
	import type { ResizeGroup } from '$lib/utils/resizeGroup';

	interface Props {
		/** Default value */
		defaultValue: number | undefined;
		/** The element that is being resized */
		viewport: HTMLElement;
		/** Sets direction of resizing for viewport */
		direction: 'left' | 'right' | 'up' | 'down';
		/** Custom z-index in case of overlapping with other elements */
		zIndex?: string;
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
		onDblClick?: () => void;
	}

	let {
		defaultValue,
		viewport,
		direction,
		zIndex = 'var(--z-lifted)',
		minWidth = 0,
		maxWidth = 120,
		minHeight = 0,
		maxHeight = 120,
		syncName,
		persistId,
		passive,
		hidden,
		resizeGroup,
		order,
		unsetMaxHeight,
		onResizing,
		onOverflow,
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

	let unsubUp: () => void;
	let unsubMove: () => void;

	function onMouseDown(e: MouseEvent) {
		e.stopPropagation();
		e.preventDefault();
		unsubUp = on(document, 'pointerup', onMouseUp);
		unsubMove = on(document, 'pointermove', onMouseMove);

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
				offsetPx = e.clientX - initial;
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
		unsubUp?.();
		unsubMove?.();
		onResizing?.(false);
	}

	function onclick(e: MouseEvent) {
		e.preventDefault();
		e.stopPropagation();
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

<div
	role="presentation"
	bind:this={resizerDiv}
	data-remove-from-draggable
	onpointerdown={onMouseDown}
	ondblclick={() => {
		onDblClick?.();
		setValue(defaultValue);
	}}
	{onclick}
	class:hidden
	class="resizer"
	class:dragging
	class:vertical={orientation === 'vertical'}
	class:horizontal={orientation === 'horizontal'}
	class:up={direction === 'up'}
	class:down={direction === 'down'}
	class:left={direction === 'left'}
	class:right={direction === 'right'}
	style:z-index={zIndex}
></div>

<style lang="postcss">
	.resizer {
		--resizer-thickness: 4px;
		--resizer-cursor: default;
		position: absolute;
		outline: none;
		cursor: var(--resizer-cursor);

		&.horizontal {
			--resizer-cursor: col-resize;
			top: 0;
			width: var(--resizer-thickness);
			height: 100%;
		}

		&.vertical {
			--resizer-cursor: row-resize;
			left: 0;
			width: 100%;
			height: var(--resizer-thickness);
		}

		&.right {
			right: 0;
		}
		&.left {
			left: 0;
		}
		&.up {
			top: 0;
		}
		&.down {
			bottom: 0;
		}

		&.hidden {
			pointer-events: none;
			--resizer-cursor: default;
		}
	}
</style>

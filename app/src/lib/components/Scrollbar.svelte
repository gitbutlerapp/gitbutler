<script lang="ts" context="module">
	export type ScrollbarPadding = { top?: number; right?: number; bottom?: number; left?: number };
</script>

<script lang="ts">
	import { SETTINGS, type Settings } from '$lib/settings/userSettings';
	import { getContextStoreBySymbol } from '$lib/utils/context';
	import { pxToRem } from '$lib/utils/pxToRem';
	import { onDestroy, createEventDispatcher } from 'svelte';

	const userSettings = getContextStoreBySymbol<Settings>(SETTINGS);

	export let viewport: Element;
	export let contents: Element;
	export let initiallyVisible = false;
	export let thickness = '0.563rem';
	export let padding: ScrollbarPadding = {};
	export let shift = '0';
	export let horz = false;
	export let zIndex = 'var(--z-lifted)';

	let thumb: Element;
	let track: Element;
	let startTop = 0;
	let startLeft = 0;
	let startY = 0;
	let startX = 0;
	let isDragging = false;

	$: teardownViewport = setupViewport(viewport);
	$: teardownThumb = setupThumb(thumb);
	$: teardownTrack = setupTrack(track);
	$: teardownContents = setupContents(contents);

	$: vert = !horz;

	$: paddingTop = pxToRem(padding.top) ?? '0px';
	$: paddingBottom = pxToRem(padding.bottom) ?? '0px';
	$: paddingRight = pxToRem(padding.right) ?? '0px';
	$: paddingLeft = pxToRem(padding.left) ?? '0px';

	$: wholeHeight = viewport?.scrollHeight ?? 0;
	$: wholeWidth = viewport?.scrollWidth ?? 0;
	$: scrollTop = viewport?.scrollTop ?? 0;
	$: scrollLeft = viewport?.scrollLeft ?? 0;
	$: trackHeight = viewport?.clientHeight ?? 0;
	$: trackWidth = viewport?.clientHeight ?? 0;
	$: thumbHeight = wholeHeight > 0 ? (trackHeight / wholeHeight) * trackHeight : 0;
	$: thumbWidth = wholeWidth > 0 ? (trackWidth / wholeWidth) * trackWidth : 0;
	$: thumbTop = wholeHeight > 0 ? (scrollTop / wholeHeight) * trackHeight : 0;
	$: thumbLeft = wholeHeight > 0 ? (scrollLeft / wholeWidth) * trackWidth : 0;

	$: scrollableY = wholeHeight > trackHeight;
	$: scrollableX = wholeWidth > trackWidth;
	$: isScrollable = scrollableY || scrollableX;
	$: shouldShowInitially = initiallyVisible && isScrollable;
	$: shouldShowOnHover = $userSettings.scrollbarVisibilityState === 'hover' && isScrollable;
	$: shouldAlwaysShow = $userSettings.scrollbarVisibilityState === 'always' && isScrollable;

	$: visible = shouldShowInitially || (shouldShowOnHover && initiallyVisible) || shouldAlwaysShow;

	const dispatch = createEventDispatcher<{
		dragging: boolean;
	}>();

	/////////////////////
	// TIMER FUNCTIONS //
	/////////////////////
	let timer = 0;

	function setupTimer() {
		if (shouldShowOnHover || shouldAlwaysShow) return;

		timer = window.setTimeout(() => {
			visible = false;
			return;
		}, 1000);
	}

	function clearTimer() {
		if (timer) {
			window.clearTimeout(timer);
			timer = 0;
		}
	}

	/////////////////////
	// VIEWPORT EVENTS //
	/////////////////////
	function onViewportMouseEnter() {
		if (!shouldShowOnHover) return;
		visible = true;
	}

	function onViewportMouseLeave() {
		if (!shouldShowOnHover) return;
		visible = false;
	}

	function setupViewport(viewport: Element) {
		if (!viewport) return;
		teardownViewport?.();

		if (typeof window.ResizeObserver === 'undefined') {
			throw new Error('window.ResizeObserver is missing.');
		}

		const observer = new ResizeObserver((entries) => {
			for (const _entry of entries) {
				wholeHeight = viewport?.scrollHeight ?? 0;
				wholeWidth = viewport?.scrollWidth ?? 0;
				trackHeight = viewport?.clientHeight ?? 0;
				trackWidth = viewport?.clientWidth;
			}
		});

		observer.observe(viewport);
		viewport.addEventListener('scroll', onScroll, { passive: true });
		viewport.addEventListener('mouseenter', onViewportMouseEnter);
		viewport.addEventListener('mouseleave', onViewportMouseLeave);

		return () => {
			observer.disconnect();
			viewport.removeEventListener('scroll', onScroll);
			viewport.removeEventListener('mouseenter', onViewportMouseEnter);
			viewport.removeEventListener('mouseleave', onViewportMouseLeave);
		};
	}

	//////////////////
	// TRACK EVENTS //
	//////////////////
	function onTrackEnter() {
		if (shouldShowOnHover || shouldAlwaysShow) return;
		clearTimer();
	}

	function onTrackLeave() {
		if (shouldShowOnHover || shouldAlwaysShow) return;

		clearTimer();
		setupTimer();
	}

	function setupTrack(track: Element) {
		if (!track) return;
		teardownTrack?.();

		track.addEventListener('mousedown', onThumbClick, { passive: true });
		track.addEventListener('mouseenter', onTrackEnter);
		track.addEventListener('mouseleave', onTrackLeave);

		return () => {
			track.removeEventListener('mousedown', onTrackClick);
			track.removeEventListener('mouseenter', onTrackEnter);
			track.removeEventListener('mouseleave', onTrackLeave);
		};
	}

	//////////////////
	// THUMB EVENTS //
	//////////////////
	function setupThumb(thumb: Element) {
		if (!thumb) return;
		teardownThumb?.();

		thumb.addEventListener('mousedown', onThumbClick, { passive: true });
		return () => {
			thumb.removeEventListener('mousedown', onThumbClick);
		};
	}

	function setupContents(contents: Element) {
		if (!contents) return;
		teardownContents?.();

		if (typeof window.ResizeObserver === 'undefined') {
			throw new Error('window.ResizeObserver is missing.');
		}
		const observer = new ResizeObserver((entries) => {
			for (const _entry of entries) {
				wholeHeight = viewport?.scrollHeight ?? 0;
				wholeWidth = viewport?.scrollWidth ?? 0;
			}
		});
		observer.observe(contents);

		return () => {
			observer.disconnect();
		};
	}

	function onScroll() {
		if (!isScrollable) return;

		clearTimer();
		setupTimer();

		visible = true;
		scrollTop = viewport?.scrollTop ?? 0;
		scrollLeft = viewport?.scrollLeft ?? 0;
	}

	function onMouseMove(event: MouseEvent) {
		event.stopPropagation();
		event.preventDefault();

		viewport.scrollTop = startTop + (wholeHeight / trackHeight) * (event.clientY - startY);
		viewport.scrollLeft = startLeft + (wholeWidth / trackWidth) * (event.clientX - startX);
	}

	function onMouseUp(event: MouseEvent) {
		event.stopPropagation();
		event.preventDefault();

		startTop = 0;
		startY = 0;

		startLeft = 0;
		startX = 0;

		isDragging = false;

		document.removeEventListener('mousemove', onMouseMove);
		document.removeEventListener('mouseup', onMouseUp);
	}

	function onTrackClick(event: Event | MouseEvent) {
		event.stopPropagation();
		event.preventDefault();

		if (event instanceof MouseEvent) {
			const clickOffsetY = (event.offsetY / trackHeight) * wholeHeight;
			const clickOffsetX = (event.offsetX / trackWidth) * wholeWidth;
			const halfThumbY = (thumbHeight / trackHeight) * (wholeHeight / 2);
			const halfThumbX = (thumbWidth / trackWidth) * (wholeWidth / 2);
			if (vert) viewport.scrollTo({ top: clickOffsetY - halfThumbY });
			if (!vert) viewport.scrollTo({ top: clickOffsetX - halfThumbX });
			startY = event.clientY;
			startTop = viewport.scrollTop;
			startX = event.clientY;
			startLeft = viewport.scrollTop;
		}

		document.addEventListener('mousemove', onMouseMove);
		document.addEventListener('mouseup', onMouseUp);
	}

	function onThumbClick(event: Event | MouseEvent) {
		event.stopPropagation();
		event.preventDefault();

		isDragging = true;

		startTop = viewport.scrollTop;
		startLeft = viewport.scrollLeft;
		if (event instanceof MouseEvent) {
			startY = event.clientY;
			startX = event.clientX;
		}

		document.addEventListener('mousemove', onMouseMove);
		document.addEventListener('mouseup', onMouseUp);
	}

	onDestroy(() => {
		teardownViewport?.();
		teardownContents?.();
		teardownThumb?.();
	});

	$: {
		dispatch('dragging', isDragging);
	}
</script>

<div
	bind:this={track}
	class="scrollbar-track"
	class:horz
	class:vert
	class:show-scrollbar={visible}
	class:thumb-dragging={isDragging}
	style:width={vert ? thickness : `100%`}
	style:height={vert ? `100%` : thickness}
	style:z-index={zIndex}
	style="
    --scrollbar-shift-vertical: {vert ? '0' : shift};
    --scrollbar-shift-horizontal: {horz ? '0' : shift};
    "
>
	<div
		bind:this={thumb}
		class="scrollbar-thumb"
		style="
          --thumb-width: {vert
			? '100%'
			: `calc(${thumbWidth.toFixed(0)}px - (${paddingRight} + ${paddingLeft}))`};
          --thumb-height: {vert
			? `calc(${thumbHeight.toFixed(0)}px - (${paddingBottom} + ${paddingTop}))`
			: '100%'};
          --thumb-top: {vert ? `calc(${thumbTop.toFixed(0)}px + ${paddingTop})` : 'auto'};
          --thumb-left: {vert ? 'auto' : `calc(${thumbLeft.toFixed(0)}px + ${paddingLeft})`};
        "
	></div>
</div>

<style>
	.scrollbar-track {
		/* scrollbar variables */
		--scrollbar-shift-vertical: 0;
		--scrollbar-shift-horizontal: 0;
		/* variable props */
		bottom: var(--scrollbar-shift-vertical);
		right: var(--scrollbar-shift-horizontal);
		/* other props */
		position: absolute;
		/* background-color: aqua; */
		transition:
			opacity 0.2s,
			width 0.1s,
			height 0.1s;
	}

	.scrollbar-thumb {
		/* variable props */
		width: var(--thumb-width);
		height: var(--thumb-height);
		top: var(--thumb-top);
		left: var(--thumb-left);
		/* other props */
		position: absolute;
		background-color: var(--clr-scale-ntrl-0);
		opacity: 0;
		transition:
			opacity 0.2s,
			transform 0.15s;
	}

	/* modify vertical scrollbar */
	.scrollbar-track.vert {
		& .scrollbar-thumb {
			transform: scaleX(0.6);
			transform-origin: right;
		}
	}

	/* modify horizontal scrollbar */
	.scrollbar-track.horz {
		& .scrollbar-thumb {
			transform: scaleY(0.65);
			transform-origin: bottom;
		}
	}

	/* MODIFIERS */

	.show-scrollbar {
		& .scrollbar-thumb {
			opacity: 0.15;
		}
	}

	/* hover state for thumb */
	.show-scrollbar:hover,
	.thumb-dragging {
		& .scrollbar-thumb {
			opacity: 0.25;
		}
	}

	.show-scrollbar.vert:hover,
	.thumb-dragging.vert {
		& .scrollbar-thumb {
			transform: scaleY(1);
		}
	}

	.show-scrollbar.horz:hover,
	.thumb-dragging.horz {
		& .scrollbar-thumb {
			transform: scaleX(1);
		}
	}
</style>

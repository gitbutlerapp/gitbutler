<script lang="ts" context="module">
	export type ScrollbarPadding = { top?: string; right?: string; bottom?: string; left?: string };
</script>

<script lang="ts">
	import { SETTINGS_CONTEXT, type SettingsStore } from '$lib/settings/userSettings';
	import { onDestroy, createEventDispatcher } from 'svelte';
	import { getContext } from 'svelte';

	const userSettings = getContext(SETTINGS_CONTEXT) as SettingsStore;

	export let viewport: Element;
	export let contents: Element;
	export let hideAfter = 1000;
	export let initiallyVisible = false;
	export let thickness = '0.563rem';
	export let padding: ScrollbarPadding = {};
	export let shift = '0';

	export let horz = false;

	// Custom z-index in case of overlapping with other elements
	export let zIndex = 20;

	$: vert = !horz;

	let thumb: Element;
	let track: Element;
	let startTop = 0;
	let startLeft = 0;
	let startY = 0;
	let startX = 0;
	let timer = 0;
	let interacted = false;

	let isViewportHovered = false;
	let isDragging = false;

	$: teardownViewport = setupViewport(viewport);
	$: teardownThumb = setupThumb(thumb);
	$: teardownTrack = setupTrack(track);
	$: teardownContents = setupContents(contents);

	$: paddingTop = padding.top ?? '0px';
	$: paddingBottom = padding.bottom ?? '0px';
	$: paddingRight = padding.right ?? '0px';
	$: paddingLeft = padding.left ?? '0px';

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

	$: alwaysVisible = $userSettings.scrollbarVisabilityOnHover;

	$: scrollableY = wholeHeight > trackHeight;
	$: scrollableX = wholeWidth > trackWidth;
	$: visible =
		((scrollableY || scrollableX) && initiallyVisible) ||
		(alwaysVisible && isViewportHovered && (scrollableY || scrollableX));

	const dispatch = createEventDispatcher<{
		dragging: boolean;
	}>();

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

		if (alwaysVisible) {
			viewport.addEventListener('mouseenter', onViewportMouseEnter);
			viewport.addEventListener('mouseleave', onViewportMouseLeave);
		}

		return () => {
			observer.disconnect();
			viewport.removeEventListener('scroll', onScroll);
		};
	}

	function onViewportMouseEnter() {
		isViewportHovered = true;
	}

	function onViewportMouseLeave() {
		isViewportHovered = false;
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

	function setupTimer() {
		timer = window.setTimeout(() => {
			visible =
				((scrollableY || scrollableX) && initiallyVisible && !interacted) ||
				(isViewportHovered && alwaysVisible) ||
				false;
		}, hideAfter);
	}

	function clearTimer() {
		if (timer) {
			window.clearTimeout(timer);
			timer = 0;
		}
	}

	function onScroll() {
		if (!scrollableY && !scrollableX) return;

		clearTimer();
		setupTimer();

		visible = alwaysVisible || (initiallyVisible && !interacted) || true;
		scrollTop = viewport?.scrollTop ?? 0;
		scrollLeft = viewport?.scrollLeft ?? 0;
		interacted = true;
	}

	function onTrackEnter() {
		clearTimer();
	}

	function onTrackLeave() {
		clearTimer();
		setupTimer();
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
	/>
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
		z-index: 30;
		background-color: var(--clr-theme-scale-ntrl-0);
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

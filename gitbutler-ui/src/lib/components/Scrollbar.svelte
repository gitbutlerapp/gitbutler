<script lang="ts">
	import { onDestroy, createEventDispatcher } from 'svelte';

	export let viewport: Element;
	export let contents: Element;
	export let hideAfter = 1000;
	export let initiallyVisible = false;
	export let thickness = 'var(--space-8)';

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

	let alwaysVisible = false;
	let isViewportHovered = false;
	let isDragging = false;

	$: teardownViewport = setupViewport(viewport);
	$: teardownThumb = setupThumb(thumb);
	$: teardownTrack = setupTrack(track);
	$: teardownContents = setupContents(contents);

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
				trackWidth = viewport?.clientWidth ?? 0;
			}
		});

		observer.observe(viewport);

		viewport.addEventListener('scroll', onScroll, { passive: true });

		if (alwaysVisible) {
			viewport.addEventListener('mouseenter', onViewportMouseEnter);
			viewport.addEventListener('mouseleave', onViewportMouseLeave);
		}

		return () => {
			observer.unobserve(contents);
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
			observer.unobserve(contents);
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
	style:right={vert ? 0 : undefined}
	style:top={vert ? 0 : undefined}
	style:bottom={horz ? 0 : undefined}
	style:left={horz ? 0 : undefined}
	style:width={vert ? thickness : `80%`}
	style:height={vert ? `80%` : thickness}
	style:z-index={zIndex}
>
	<div
		bind:this={thumb}
		class="scrollbar-thumb"
		style:left={vert ? undefined : `${thumbLeft}px`}
		style:top={vert ? `${thumbTop}px` : undefined}
		style:width={vert ? '100%' : `${thumbWidth}px`}
		style:height={vert ? `${thumbHeight}px` : '100%'}
	/>
</div>

<style>
	.scrollbar-track {
		position: absolute;
		transition:
			opacity 0.2s,
			width 0.1s,
			height 0.1s;
	}

	.scrollbar-thumb {
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
			transform: scaleY(0.6);
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

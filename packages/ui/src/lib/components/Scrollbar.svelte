<script lang="ts">
	import { onDestroy } from 'svelte';

	export let viewport: Element;
	export let contents: Element;
	export let hideAfter = 1000;
	export let alwaysVisible = false;
	export let initiallyVisible = false;
	export let margin: { top?: number; right?: number; bottom?: number; left?: number } = {};
	export let opacity = '0.2';
	export let thickness = '0.625rem';
	export let vertical = false;

	$: horz = !vertical;

	let thumb: Element;
	let track: Element;
	let startTop = 0;
	let startLeft = 0;
	let startY = 0;
	let startX = 0;
	let timer = 0;
	let interacted = false;

	$: teardownViewport = setupViewport(viewport);
	$: teardownThumb = setupThumb(thumb);
	$: teardownTrack = setupTrack(track);
	$: teardownContents = setupContents(contents);

	$: marginTop = margin.top ?? 0;
	$: marginBottom = margin.bottom ?? 0;
	$: marginRight = margin.right ?? 0;
	$: marginLeft = margin.left ?? 0;

	$: wholeHeight = viewport?.scrollHeight ?? 0;
	$: wholeWidth = viewport?.scrollWidth ?? 0;
	$: scrollTop = viewport?.scrollTop ?? 0;
	$: scrollLeft = viewport?.scrollLeft ?? 0;
	$: trackHeight = viewport?.clientHeight ?? 0 - (marginTop + marginBottom);
	$: trackWidth = viewport?.clientHeight ?? 0 - (marginTop + marginBottom);
	$: thumbHeight = wholeHeight > 0 ? (trackHeight / wholeHeight) * trackHeight : 0;
	$: thumbWidth = wholeWidth > 0 ? (trackWidth / wholeWidth) * trackWidth : 0;
	$: thumbTop = wholeHeight > 0 ? (scrollTop / wholeHeight) * trackHeight : 0;
	$: thumbLeft = wholeHeight > 0 ? (scrollLeft / wholeWidth) * trackWidth : 0;

	$: scrollableY = wholeHeight > trackHeight;
	$: scrollableX = wholeWidth > trackWidth;
	$: visible = (scrollableY || scrollableX) && (alwaysVisible || initiallyVisible);

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
				trackHeight = viewport?.clientHeight - (marginTop + marginBottom) ?? 0;
				trackWidth = viewport?.clientWidth - (marginLeft + marginRight) ?? 0;
			}
		});
		observer.observe(viewport);

		viewport.addEventListener('scroll', onScroll, { passive: true });

		return () => {
			observer.unobserve(contents);
			observer.disconnect();
			viewport.removeEventListener('scroll', onScroll);
		};
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
				((scrollableY || scrollableX) && (alwaysVisible || (initiallyVisible && !interacted))) ||
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
			if (horz) viewport.scrollTo({ top: clickOffsetY - halfThumbY });
			if (!horz) viewport.scrollTo({ top: clickOffsetX - halfThumbX });
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

		document.removeEventListener('mousemove', onMouseMove);
		document.removeEventListener('mouseup', onMouseUp);
	}

	onDestroy(() => {
		teardownViewport?.();
		teardownContents?.();
		teardownThumb?.();
	});
</script>

<div
	bind:this={track}
	class="absolute top-0 duration-200"
	class:right-0={horz}
	class:top-0={horz}
	class:bottom-0={vertical}
	class:left-0={vertical}
	style:width={horz ? thickness : `${trackWidth}px`}
	style:height={horz ? `${trackHeight}px` : thickness}
	style:margin={`${marginTop}rem ${marginRight}rem ${marginBottom}rem ${marginLeft}rem`}
>
	<div
		bind:this={thumb}
		class="absolute z-30 bg-black transition-opacity dark:bg-white"
		style:opacity={visible ? opacity : 0}
		style:left={horz ? undefined : `${thumbLeft}px`}
		style:top={horz ? `${thumbTop}px` : undefined}
		style:width={horz ? thickness : `${thumbWidth}px`}
		style:height={horz ? `${thumbHeight}px` : thickness}
	/>
</div>

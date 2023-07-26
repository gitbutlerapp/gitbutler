<script lang="ts">
	import { onDestroy, onMount } from 'svelte';

	export let viewport: Element;
	export let contents: Element;
	export let hideAfter = 1000;
	export let alwaysVisible = false;
	export let initiallyVisible = false;
	export let margin: { top?: number; right?: number; bottom?: number; left?: number } = {};
	export let opacity: string = '0.2';
	export let width: string = '0.625rem';

	let thumb: Element;
	let track: Element;
	let startTop = 0;
	let startY = 0;
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
	$: scrollTop = viewport?.scrollTop ?? 0;
	$: trackHeight = viewport?.clientHeight ?? 0 - (marginTop + marginBottom);
	$: thumbHeight = wholeHeight > 0 ? (trackHeight / wholeHeight) * trackHeight : 0;
	$: thumbTop = wholeHeight > 0 ? (scrollTop / wholeHeight) * trackHeight : 0;

	$: scrollable = wholeHeight > trackHeight;
	$: visible = scrollable && (alwaysVisible || initiallyVisible);

	function setupViewport(viewport: Element) {
		if (!viewport) return;
		teardownViewport?.();

		if (typeof window.ResizeObserver === 'undefined') {
			throw new Error('window.ResizeObserver is missing.');
		}
		const observer = new ResizeObserver((entries) => {
			for (const _entry of entries) {
				wholeHeight = viewport?.scrollHeight ?? 0;
				trackHeight = viewport?.clientHeight - (marginTop + marginBottom) ?? 0;
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
			track.removeEventListener('mousedown', onThumbClick);
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
			visible = (scrollable && (alwaysVisible || (initiallyVisible && !interacted))) || false;
		}, hideAfter);
	}

	function clearTimer() {
		if (timer) {
			window.clearTimeout(timer);
			timer = 0;
		}
	}

	function onScroll() {
		if (!scrollable) return;

		clearTimer();
		setupTimer();

		visible = alwaysVisible || (initiallyVisible && !interacted) || true;
		scrollTop = viewport?.scrollTop ?? 0;
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
			const clickOffset = (event.offsetY / trackHeight) * wholeHeight;
			const halfThumb = (thumbHeight / trackHeight) * (wholeHeight / 2);
			viewport.scrollTo({ top: clickOffset - halfThumb });
			startY = event.clientY;
			startTop = viewport.scrollTop;
		}

		document.addEventListener('mousemove', onMouseMove);
		document.addEventListener('mouseup', onMouseUp);
	}

	function onThumbClick(event: Event | MouseEvent) {
		event.stopPropagation();
		event.preventDefault();

		startTop = viewport.scrollTop;
		if (event instanceof MouseEvent) {
			startY = event.clientY;
		}

		document.addEventListener('mousemove', onMouseMove);
		document.addEventListener('mouseup', onMouseUp);
	}

	function onMouseMove(event: MouseEvent) {
		event.stopPropagation();
		event.preventDefault();

		viewport.scrollTop = startTop + (wholeHeight / trackHeight) * (event.clientY - startY);
	}

	function onMouseUp(event: MouseEvent) {
		event.stopPropagation();
		event.preventDefault();

		startTop = 0;
		startY = 0;

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
	class="absolute right-0 top-0 transition-opacity duration-200"
	style:width
	style:height={`${trackHeight}px`}
	style:opacity={visible ? '1' : '0'}
	style:margin={`${marginTop}rem ${marginRight}rem ${marginBottom}rem ${marginLeft}rem`}
>
	<div
		bind:this={thumb}
		class="absolute bg-black dark:bg-white"
		style:width
		style:opacity
		style:top={`${thumbTop}px`}
		style:height={`${thumbHeight}px`}
	/>
</div>

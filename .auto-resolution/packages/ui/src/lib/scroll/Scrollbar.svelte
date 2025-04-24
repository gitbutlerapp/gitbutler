<script lang="ts" module>
	export type ScrollbarPaddingType = {
		top?: number;
		right?: number;
		bottom?: number;
		left?: number;
	};
</script>

<script lang="ts">
	import { pxToRem } from '$lib/utils/pxToRem';

	interface Props {
		viewport: HTMLElement;
		initiallyVisible?: boolean;
		thickness?: string;
		padding?: ScrollbarPaddingType;
		shift?: string;
		horz?: boolean;
		zIndex?: string;
		whenToShow: 'hover' | 'always' | 'scroll';
		onthumbdrag?: (dragging: boolean) => void;
		onscroll?: (e: Event) => void;
	}

	const {
		viewport,
		initiallyVisible = false,
		thickness = '0.563rem',
		padding = {},
		shift = '0',
		horz = false,
		zIndex = 'var(--z-lifted)',
		whenToShow = 'hover',
		onthumbdrag,
		onscroll
	}: Props = $props();

	$effect(() => {
		if (viewport) {
			setupViewport(viewport);
		}

		if (thumb) {
			setupThumb(thumb);
		}

		if (track) {
			setupTrack(track);
		}
	});

	$effect(() => {
		onthumbdrag?.(isDragging);
	});

	let thumb: Element | undefined = $state();
	let track: Element | undefined = $state();
	let startTop = $state(0);
	let startLeft = $state(0);
	let startY = $state(0);
	let startX = $state(0);
	let isDragging = $state(false);

	const vert = $derived(!horz);

	const paddingTop = $derived(pxToRem(padding.top ?? 0));
	const paddingBottom = $derived(pxToRem(padding.bottom ?? 0));
	const paddingRight = $derived(pxToRem(padding.right ?? 0));
	const paddingLeft = $derived(pxToRem(padding.left ?? 0));

	let wholeHeight = $state(viewport?.scrollHeight ?? 0);
	let wholeWidth = $state(viewport?.scrollWidth ?? 0);
	let scrollTop = $state(viewport?.scrollTop ?? 0);
	let scrollLeft = $state(viewport?.scrollLeft ?? 0);
	let trackHeight = $state(viewport?.offsetHeight ?? 0);
	let trackWidth = $state(viewport?.offsetWidth ?? 0);

	const thumbHeight = $derived(wholeHeight > 0 ? (trackHeight / wholeHeight) * trackHeight : 0);
	const thumbWidth = $derived(wholeWidth > 0 ? (trackWidth / wholeWidth) * trackWidth : 0);
	const thumbTop = $derived(wholeHeight > 0 ? (scrollTop / wholeHeight) * trackHeight : 0);
	const thumbLeft = $derived(wholeHeight > 0 ? (scrollLeft / wholeWidth) * trackWidth : 0);

	const scrollableY = $derived(wholeHeight > trackHeight);
	const scrollableX = $derived(wholeWidth > trackWidth);
	const isScrollable = $derived(scrollableY || scrollableX);
	const shouldShowInitially = $derived(initiallyVisible && isScrollable);

	const shouldShowOnHover = $derived(whenToShow === 'hover' && isScrollable);
	const shouldAlwaysShow = $derived(whenToShow === 'always' && isScrollable);

	let visible = $state(false);

	// let visible = $state(false);

	$effect(() => {
		visible = shouldShowInitially || (shouldShowOnHover && initiallyVisible) || shouldAlwaysShow;
	});

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

		if (typeof window.ResizeObserver === 'undefined') {
			throw new Error('window.ResizeObserver is missing.');
		}

		const observerSize = new ResizeObserver(updateTrack);
		observerSize.observe(viewport);

		const observerMutations = new MutationObserver(updateTrack);
		observerMutations.observe(viewport, { childList: true, subtree: true });

		viewport.addEventListener('scroll', onScroll, { passive: true });
		viewport.addEventListener('mouseenter', onViewportMouseEnter);
		viewport.addEventListener('mouseleave', onViewportMouseLeave);

		return () => {
			observerSize.disconnect();
			observerMutations.disconnect();
			viewport.removeEventListener('scroll', onScroll);
			viewport.removeEventListener('mouseenter', onViewportMouseEnter);
			viewport.removeEventListener('mouseleave', onViewportMouseLeave);
		};
	}

	function updateTrack() {
		wholeHeight = viewport?.scrollHeight ?? 0;
		wholeWidth = viewport?.scrollWidth ?? 0;
		trackHeight = viewport?.clientHeight ?? 0;
		trackWidth = viewport?.clientWidth ?? 0;
	}

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

		thumb.addEventListener('mousedown', onThumbClick, { passive: true });
		return () => {
			thumb.removeEventListener('mousedown', onThumbClick);
		};
	}

	function onScroll(e: Event) {
		if (!isScrollable) return;

		onscroll?.(e);

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
</script>

<div
	bind:this={track}
	data-remove-from-draggable
	class="scrollbar-track"
	class:horz
	class:vert
	class:show-scrollbar={visible}
	class:thumb-dragging={isDragging && visible}
	style:width={vert ? thickness : `100%`}
	style:height={vert ? `100%` : thickness}
	style:z-index={zIndex}
	style="
    --scrollbar-shift-vertical: {vert ? '0' : shift || 0};
    --scrollbar-shift-horizontal: {horz ? '0' : shift || 0};
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
	/* general */
	.show-scrollbar:hover,
	.thumb-dragging {
		& .scrollbar-thumb {
			opacity: 0.25;
			transition:
				opacity 0.2s,
				transform 0.15s;
		}
	}
	/* vertical */
	.show-scrollbar.vert:hover,
	.thumb-dragging.vert {
		& .scrollbar-thumb {
			transform: scaleY(1);
		}
	}
	/* horizontal */
	.show-scrollbar.horz:hover,
	.thumb-dragging.horz {
		& .scrollbar-thumb {
			transform: scaleX(1);
		}
	}
</style>

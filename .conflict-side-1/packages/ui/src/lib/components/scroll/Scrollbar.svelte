<script lang="ts" module>
	export type ScrollbarVisilitySettings = 'scroll' | 'hover' | 'always';
</script>

<script lang="ts">
	interface Props {
		viewport: HTMLElement;
		initiallyVisible?: boolean;
		thickness?: string;
		shift?: string;
		horz?: boolean;
		zIndex?: string;
		whenToShow: ScrollbarVisilitySettings;
		onthumbdrag?: (dragging: boolean) => void;
		onscroll?: (e: Event) => void;
		onscrollexists?: (hasScroll: boolean) => void;
	}

	const {
		viewport,
		initiallyVisible = false,
		thickness = '.5rem',
		shift = '0',
		horz = false,
		zIndex = 'var(--z-ground)',
		whenToShow = 'hover',
		onthumbdrag,
		onscroll,
		onscrollexists
	}: Props = $props();

	$effect(() => {
		if (viewport) {
			return setupViewport(viewport);
		}
	});

	$effect(() => {
		if (thumb) {
			return setupThumb(thumb);
		}
	});

	$effect(() => {
		if (track) {
			return setupTrack(track);
		}
	});

	$effect(() => {
		onthumbdrag?.(isDragging);
	});

	// New effect to call onscrollexists when scroll state changes
	$effect(() => {
		onscrollexists?.(isScrollable);
	});

	let thumb: Element | undefined = $state();
	let track: Element | undefined = $state();
	let startTop = $state(0);
	let startLeft = $state(0);
	let startY = $state(0);
	let startX = $state(0);
	let isDragging = $state(false);

	const vert = $derived(!horz);

	let wholeHeight = $state(viewport?.scrollHeight ?? 0);
	let wholeWidth = $state(viewport?.scrollWidth ?? 0);
	let scrollTop = $state(viewport?.scrollTop ?? 0);
	let scrollLeft = $state(viewport?.scrollLeft ?? 0);
	let trackHeight = $state(viewport?.offsetHeight ?? 0);
	let trackWidth = $state(viewport?.offsetWidth ?? 0);

	const thumbHeight = $derived(wholeHeight > 0 ? (trackHeight / wholeHeight) * trackHeight : 0);
	const thumbWidth = $derived(wholeWidth > 0 ? (trackWidth / wholeWidth) * trackWidth : 0);
	const thumbTop = $derived(
		wholeHeight > 0 ? ((scrollTop + trackHeight) / wholeHeight) * trackHeight - thumbHeight : 0
	);
	const thumbLeft = $derived(
		wholeWidth > 0 ? ((scrollLeft + trackWidth) / wholeWidth) * trackWidth - thumbWidth : 0
	);

	const scrollableY = $derived(wholeHeight > trackHeight);
	const scrollableX = $derived(wholeWidth > trackWidth);
	const isScrollable = $derived(scrollableY || scrollableX);
	const shouldShowInitially = $derived(initiallyVisible && isScrollable);

	const shouldShowOnHover = $derived(whenToShow === 'hover' && isScrollable);
	const shouldAlwaysShow = $derived(whenToShow === 'always' && isScrollable);

	let visible = $derived(
		shouldShowInitially || (shouldShowOnHover && initiallyVisible) || shouldAlwaysShow
	);

	// Used to detect sudden changes to content height.
	let lastHeight: number | undefined;

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

		const observerSize = new ResizeObserver(() => updateTrack());
		observerSize.observe(viewport);

		const content = viewport.children.item(0);
		if (!content) {
			throw new Error('Expected to find content container');
		}

		// Sometimes the content size changes before scrollTop, so we
		// compensate here to avoid jumpiness. The position will be reset
		// on next scroll event.
		const observerContentSize = new ResizeObserver(() => {
			if (lastHeight) {
				const diff = content.scrollHeight - lastHeight;
				scrollTop = viewport.scrollTop + diff;
			}
			lastHeight = content.scrollHeight;
		});
		observerContentSize.observe(content);

		const observerMutations = new MutationObserver(updateTrack);
		observerMutations.observe(viewport, { childList: true, subtree: true });

		viewport.addEventListener('scroll', onScroll, { passive: true });
		viewport.addEventListener('mouseenter', onViewportMouseEnter);
		viewport.addEventListener('mouseleave', onViewportMouseLeave);

		return () => {
			observerSize.disconnect();
			observerMutations.disconnect();
			observerContentSize.disconnect();
			viewport.removeEventListener('scroll', onScroll);
			viewport.removeEventListener('mouseenter', onViewportMouseEnter);
			viewport.removeEventListener('mouseleave', onViewportMouseLeave);
		};
	}

	export function updateTrack() {
		wholeHeight = viewport?.scrollHeight ?? 0;
		wholeWidth = viewport?.scrollWidth ?? 0;
		trackHeight = viewport?.offsetHeight ?? 0;
		trackWidth = viewport?.offsetWidth ?? 0;
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
	data-remove-from-panning
	data-no-drag
	data-drag-clone-ignore
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
          --thumb-width: {vert ? '100%' : `calc(${thumbWidth.toFixed(0)}px)`};
          --thumb-height: {vert ? `calc(${thumbHeight.toFixed(0)}px)` : '100%'};
          --thumb-top: {vert ? `${thumbTop.toFixed(0)}px` : 'auto'};
          --thumb-left: {vert ? 'auto' : `${thumbLeft.toFixed(0)}px`};
        "
	></div>
</div>

<style>
	.scrollbar-track {
		position: absolute;
		/* scrollbar variables */
		--scrollbar-shift-vertical: 0;
		--scrollbar-shift-horizontal: 0;
		right: var(--scrollbar-shift-horizontal);
		/* variable props */
		bottom: var(--scrollbar-shift-vertical);
		/* background-color: rgba(0, 0, 255, 0.1); */
	}

	.scrollbar-thumb {
		/* other props */
		position: absolute;
		top: var(--thumb-top);
		left: var(--thumb-left);
		/* variable props */
		width: var(--thumb-width);
		height: var(--thumb-height);
		background-color: var(--clr-scale-ntrl-0);
		opacity: 0;
		will-change: transform, opacity;
	}

	/* modify vertical scrollbar */
	.scrollbar-track.vert {
		& .scrollbar-thumb {
			transform: scaleX(0.7) translateZ(0);
			transform-origin: right;
		}
	}

	/* modify horizontal scrollbar */
	.scrollbar-track.horz {
		& .scrollbar-thumb {
			transform: scaleY(0.7) translateZ(0);
			transform-origin: bottom;
		}
	}

	/* MODIFIERS */

	.show-scrollbar {
		& .scrollbar-thumb {
			opacity: 0.15;
			transition:
				opacity 0.2s,
				transform 0.15s,
				height 0.15s,
				top 0.05s;
		}
	}

	/* hover state for thumb */
	/* general */
	.show-scrollbar:hover,
	.thumb-dragging {
		& .scrollbar-thumb {
			opacity: 0.25;
		}
	}
	/* vertical */
	.show-scrollbar.vert:hover,
	.thumb-dragging.vert {
		& .scrollbar-thumb {
			transform: scaleY(1) translateZ(0);
		}
	}
	/* horizontal */
	.show-scrollbar.horz:hover,
	.thumb-dragging.horz {
		& .scrollbar-thumb {
			transform: scaleX(1) translateZ(0);
		}
	}
</style>

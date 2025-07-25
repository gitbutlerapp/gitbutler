<script lang="ts" module>
	export interface ScrollableProps {
		maxHeight?: string;
		initiallyVisible?: boolean;
		wide?: boolean;
		padding?: ScrollbarPaddingType;
		shift?: string;
		thickness?: string;
		horz?: boolean;
		zIndex?: string;
		whenToShow?: 'hover' | 'always' | 'scroll';
		autoScroll?: boolean;
		onthumbdrag?: (dragging: boolean) => void;
		children: Snippet;
		onscrollTop?: (visible: boolean) => void;
		onscrollEnd?: (visible: boolean) => void;
		onscroll?: (e: Event) => void;
		onscrollexists?: (exists: boolean) => void;
		viewport?: HTMLDivElement;
		viewportHeight?: number;
		childrenWrapHeight?: string;
		/** used only with virtual list. */
		top?: number;
		bottom?: number;
	}
</script>

<script lang="ts">
	import Scrollbar, { type ScrollbarPaddingType } from '$components/scroll/Scrollbar.svelte';
	import { useAutoScroll } from '$lib/utils/autoscroll';
	import type { Snippet } from 'svelte';

	let {
		maxHeight,
		initiallyVisible,
		wide,
		padding,
		shift,
		thickness,
		horz,
		whenToShow = 'hover',
		autoScroll,
		children,
		onthumbdrag,
		onscroll,
		onscrollTop,
		onscrollEnd,
		onscrollexists,
		zIndex,
		top,
		bottom,
		viewport = $bindable(),
		viewportHeight = $bindable(),
		childrenWrapHeight
	}: ScrollableProps = $props();

	let scrollTopVisible = $state<boolean>(true);
	let scrollEndVisible = $state<boolean>(true);

	// Function to check scroll position and update visibility states
	function checkScrollPosition() {
		if (!viewport) return;

		const { scrollTop, scrollHeight, clientHeight } = viewport;
		const threshold = 1; // Small threshold to account for sub-pixel scrolling

		// Check if we're at the top
		const atTop = scrollTop <= threshold;
		scrollTopVisible = atTop;

		// Check if we're at the bottom
		const atBottom = scrollTop + clientHeight >= scrollHeight - threshold;
		scrollEndVisible = atBottom;
	}

	// Handle scroll events
	function handleScroll(e: Event) {
		checkScrollPosition();
		onscroll?.(e);
	}

	// Check initial position when viewport is available
	$effect(() => {
		if (viewport) {
			checkScrollPosition();
		}
	});

	$effect(() => {
		if (scrollTopVisible) {
			onscrollTop?.(true);
		} else {
			onscrollTop?.(false);
		}
	});

	$effect(() => {
		if (scrollEndVisible) {
			onscrollEnd?.(true);
		} else {
			onscrollEnd?.(false);
		}
	});
</script>

<div class="scrollable" style:flex-grow={wide ? 1 : 0} style:max-height={maxHeight}>
	<div
		bind:this={viewport}
		use:useAutoScroll={{ enabled: autoScroll }}
		bind:offsetHeight={viewportHeight}
		onscroll={handleScroll}
		class="viewport hide-native-scrollbar"
		style="padding-top: {top}px; padding-bottom: {bottom}px;"
	>
		<div class="children-wrap hide-native-scrollbar" style:height={childrenWrapHeight}>
			{@render children()}
		</div>
		<Scrollbar
			{whenToShow}
			{viewport}
			{initiallyVisible}
			{padding}
			{shift}
			{thickness}
			{zIndex}
			{horz}
			{onscrollexists}
			{onthumbdrag}
		/>
	</div>
</div>

<style lang="postcss">
	.scrollable {
		display: flex;
		position: relative;
		flex-direction: column;
		height: 100%;
		overflow: hidden;
	}
	.viewport {
		display: flex;
		flex-direction: column;
		width: 100%;
		height: 100%;
		overflow-y: auto;
	}

	.children-wrap {
		/* Having this be `display: content` seems to trigger excessive layout
		   computations that makes resizing the viewport really slow. */
		display: block;
	}
</style>

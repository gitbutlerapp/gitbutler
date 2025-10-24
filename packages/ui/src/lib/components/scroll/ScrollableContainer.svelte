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
		childrenWrapDisplay?: 'block' | 'content'; // 'content' is used for virtual lists to avoid unnecessary height calculations
		/** used only with virtual list. */
		top?: number;
		bottom?: number;
	}
</script>

<script lang="ts">
	import Scrollbar, { type ScrollbarPaddingType } from '$components/scroll/Scrollbar.svelte';
	import { onDestroy } from 'svelte';
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
		childrenWrapHeight,
		childrenWrapDisplay = 'block'
	}: ScrollableProps = $props();

	let scrollTopVisible = $state<boolean>(true);
	let scrollEndVisible = $state<boolean>(true);
	let rafId: number | null = null;

	// Function to check scroll position and update visibility states
	function checkScrollPosition() {
		if (!viewport) return;

		const { scrollTop, scrollHeight, clientHeight } = viewport;
		const threshold = 1; // Small threshold to account for sub-pixel scrolling

		// Check if we're at the top
		const atTop = scrollTop <= threshold;
		const prevScrollTopVisible = scrollTopVisible;
		scrollTopVisible = atTop;

		// Check if we're at the bottom
		const atBottom = scrollTop + clientHeight >= scrollHeight - threshold;
		const prevScrollEndVisible = scrollEndVisible;
		scrollEndVisible = atBottom;

		// Only call callbacks if state actually changed
		if (prevScrollTopVisible !== scrollTopVisible) {
			onscrollTop?.(scrollTopVisible);
		}
		if (prevScrollEndVisible !== scrollEndVisible) {
			onscrollEnd?.(scrollEndVisible);
		}
	}

	// Handle scroll events with RAF throttling
	function handleScroll(e: Event) {
		if (rafId) return; // Skip if already scheduled

		rafId = requestAnimationFrame(() => {
			checkScrollPosition();
			onscroll?.(e);
			rafId = null;
		});
	}

	// Check initial position when viewport is available
	$effect(() => {
		if (viewport) {
			checkScrollPosition();
		}
	});

	// Cleanup RAF on component destroy
	onDestroy(() => {
		if (rafId) {
			cancelAnimationFrame(rafId);
			rafId = null;
		}
	});

	// Export methods to programmatically control scroll position
	export function scrollTo(options: ScrollToOptions) {
		if (viewport) {
			viewport.scrollTo(options);
		}
	}

	export function scrollToTop() {
		scrollTo({ top: 0, behavior: 'smooth' });
	}

	export function scrollToBottom(immediate?: boolean) {
		if (viewport) {
			viewport.scrollTop = viewport.scrollHeight - viewport.offsetHeight;
			scrollTo({
				top: viewport.scrollHeight,
				behavior: immediate ? undefined : 'smooth'
			});
		}
	}
</script>

<div class="scrollable" style:flex-grow={wide ? 1 : 0} style:max-height={maxHeight}>
	<div
		bind:this={viewport}
		bind:offsetHeight={viewportHeight}
		style:flex-grow={wide ? 1 : 0}
		onscroll={handleScroll}
		class="viewport hide-native-scrollbar"
		style:padding-top={top + 'px'}
		style:padding-bottom={bottom + 'px'}
		style:padding-left={padding?.left ? padding.left + 'px' : undefined}
		style:padding-right={padding?.right ? padding.right + 'px' : undefined}
		style:--overflow-x={horz ? 'auto' : 'hidden'}
		style:--overflow-y={horz ? 'hidden' : 'auto'}
		style:--flex-direction={horz ? 'row' : 'column'}
	>
		<div style:min-height={childrenWrapHeight} style:display={childrenWrapDisplay}>
			{@render children()}
		</div>
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
		position: relative;
		flex-direction: column;
		width: 100%;
		height: 100%;
		overflow-x: var(--overflow-x, hidden);
		overflow-y: var(--overflow-y, auto);
	}
</style>

<script lang="ts">
	import Scrollbar, { type ScrollbarPaddingType } from '$lib/scroll/Scrollbar.svelte';
	import { useAutoScroll } from '$lib/utils/autoscroll';
	import type { Snippet } from 'svelte';

	interface Props {
		height?: string;
		maxHeight?: string;
		initiallyVisible?: boolean;
		wide?: boolean;
		padding?: ScrollbarPaddingType;
		shift?: string;
		thickness?: string;
		horz?: boolean;
		whenToShow: 'hover' | 'always' | 'scroll';
		autoScroll?: boolean;
		onthumbdrag?: (dragging: boolean) => void;
		children: Snippet;
		onscrollTop?: (visible: boolean) => void;
		onscrollEnd?: (visible: boolean) => void;
		onscroll?: (e: Event) => void;
		viewport?: HTMLDivElement;
		viewportHeight?: number;
		/** Top padding, used only with virtual list. */
		top?: number;
		/** Bottom padding, used with virtual list. */
		bottom?: number;
	}

	let {
		height,
		maxHeight,
		initiallyVisible,
		wide,
		padding,
		shift,
		thickness,
		horz,
		whenToShow,
		autoScroll,
		children,
		onthumbdrag,
		onscroll,
		onscrollTop,
		onscrollEnd,
		viewport = $bindable(),
		top,
		bottom,
		viewportHeight = $bindable()
	}: Props = $props();

	let scrollTopVisible = $state<boolean>(true);
	let scrollEndVisible = $state<boolean>(true);

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
		{onscroll}
		class="viewport hide-native-scrollbar"
		style="padding-top: {top}px; padding-bottom: {bottom}px;"
		style:height
	>
		{@render children()}
	</div>
	<Scrollbar
		{whenToShow}
		{viewport}
		{initiallyVisible}
		{padding}
		{shift}
		{thickness}
		{horz}
		{onthumbdrag}
	/>
</div>

<style lang="postcss">
	.scrollable {
		display: flex;
		flex-direction: column;
		position: relative;
		overflow: hidden;
		height: 100%;
	}
	.viewport {
		overflow-y: auto;
		height: 100%;
		width: 100%;
	}
</style>

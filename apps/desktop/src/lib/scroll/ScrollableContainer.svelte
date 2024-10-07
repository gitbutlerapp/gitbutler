<script lang="ts">
	import Scrollbar, { type ScrollbarPaddingType } from './Scrollbar.svelte';
	import { type Snippet } from 'svelte';
	import { onDestroy, onMount } from 'svelte';

	interface Props {
		height?: string;
		fillViewport?: boolean;
		maxHeight?: string;
		initiallyVisible?: boolean;
		wide?: boolean;
		padding?: ScrollbarPaddingType;
		shift?: string;
		thickness?: string;
		horz?: boolean;
		onthumbdrag?: (dragging: boolean) => void;
		children: Snippet;
		onscrollEnd?: (visible: boolean) => void;
		onscroll?: (e: Event) => void;
	}

	let {
		height,
		fillViewport,
		maxHeight,
		initiallyVisible,
		wide,
		padding,
		shift,
		thickness,
		horz,
		children,
		onthumbdrag,
		onscroll,
		onscrollEnd
	}: Props = $props();

	let viewport = $state<HTMLDivElement>();
	let contents = $state<HTMLDivElement>();
	let scrollable = $state<boolean>();
	let scrollEndVisible = $state<boolean>(false);

	let observer: ResizeObserver;

	onMount(() => {
		observer = new ResizeObserver(() => {
			if (viewport && contents) {
				scrollable = viewport.offsetHeight < contents.offsetHeight;
			}
		});
		if (viewport) observer.observe(viewport);
		if (contents) observer.observe(contents);
	});

	onDestroy(() => observer.disconnect());

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
		class="viewport hide-native-scrollbar"
		style:height
		style:overflow-y={scrollable ? 'auto' : 'hidden'}
		onscroll={(e) => {
			const target = e.target as HTMLDivElement;
			scrollEndVisible = target.scrollTop + target.clientHeight >= target.scrollHeight;

			onscroll?.(e);
		}}
	>
		<div bind:this={contents} class="contents" class:fill-viewport={fillViewport}>
			{@render children()}
		</div>
		<Scrollbar
			{viewport}
			{contents}
			{initiallyVisible}
			{padding}
			{shift}
			{thickness}
			{horz}
			{onthumbdrag}
		/>
	</div>
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
		height: 100%;
		width: 100%;
	}
	.contents {
		display: flex;
		flex-direction: column;
		min-height: 100%;
		min-width: 100%;
	}

	/* MODIFIERS */
	.fill-viewport {
		min-height: 100%;
		min-width: 100%;
	}
</style>

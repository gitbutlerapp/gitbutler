<script lang="ts">
	import Scrollbar, { type ScrollbarPadding } from '$lib/shared/Scrollbar.svelte';
	import { onDestroy, onMount, createEventDispatcher } from 'svelte';

	export const height: string | undefined = undefined;
	export let viewport: HTMLDivElement | undefined = undefined;
	export let contents: HTMLDivElement | undefined = undefined;
	export let fillViewport: boolean = false;
	export let maxHeight: string | undefined = undefined;
	export let scrollable: boolean | undefined = undefined;

	export let scrolled = false;
	export let wide = false;
	export let initiallyVisible = false;
	export let showBorderWhenScrolled = false;

	export let padding: ScrollbarPadding = {};
	export let shift = '0';
	export let thickness = '0.563rem';

	let observer: ResizeObserver;

	const dispatch = createEventDispatcher<{ dragging: boolean }>();

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
</script>

<div
	class="scrollable"
	class:scrolled={showBorderWhenScrolled && scrolled}
	style:flex-grow={wide ? 1 : 0}
	style:max-height={maxHeight}
>
	<div
		bind:this={viewport}
		class="viewport hide-native-scrollbar"
		style:height
		style:overflow-y={scrollable ? 'auto' : 'hidden'}
	>
		<div bind:this={contents} class="contents" class:fill-viewport={fillViewport}>
			<slot />
		</div>
		<Scrollbar
			{viewport}
			{contents}
			{initiallyVisible}
			{padding}
			{shift}
			{thickness}
			on:dragging={(e) => dispatch('dragging', e.detail)}
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
		display: block;
		min-width: 100%;
	}
	.scrolled {
		border-top: 1px solid var(--clr-border-2);
	}

	/* MODIFIERS */
	.fill-viewport {
		display: initial;
		min-height: 100%;
		min-width: 100%;
	}
</style>

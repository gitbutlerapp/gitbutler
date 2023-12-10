<script lang="ts">
	import Scrollbar from '$lib/components/Scrollbar.svelte';
	import { onDestroy, onMount } from 'svelte';

	export let viewport: HTMLDivElement | undefined = undefined;
	export let contents: HTMLDivElement | undefined = undefined;
	export let height: string | undefined = undefined;
	export let minHeight: string | undefined = undefined;
	export let scrollable: boolean | undefined = undefined;
	export let maxHeight: number | undefined = undefined;
	export let scrolled = false;
	export let wide = false;
	export let initiallyVisible = false;

	let observer: ResizeObserver;

	onMount(() => {
		observer = new ResizeObserver(() => {
			if (viewport && contents) {
				scrollable = viewport.offsetHeight < contents.offsetHeight;
				maxHeight = contents.offsetHeight;
			}
		});
		if (viewport) observer.observe(viewport);
		if (contents) observer.observe(contents);
	});

	onDestroy(() => observer.disconnect());
</script>

<div class="scrollable" style:flex-grow={wide ? 1 : 0}>
	<div
		bind:this={viewport}
		on:scroll={(e) => {
			scrolled = e.currentTarget.scrollTop != 0;
		}}
		class="viewport hide-native-scrollbar"
		style:height
		style:min-height={minHeight}
		style:overflow-y={scrollable ? 'scroll' : 'hidden'}
	>
		<div bind:this={contents} class="contents">
			<slot />
		</div>
	</div>
	<Scrollbar {viewport} {contents} thickness="0.4rem" {initiallyVisible} />
</div>

<style lang="postcss">
	.scrollable {
		display: flex;
		flex-direction: column;
		position: relative;
		overflow: hidden;
	}
	.viewport {
		overscroll-behavior: none;
		height: 100%;
		width: 100%;
	}
	.contents {
		display: block;
		min-width: 100%;
	}
</style>

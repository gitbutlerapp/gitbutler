<script lang="ts">
	import Scrollbar from '$lib/components/Scrollbar.svelte';
	import { onDestroy, onMount, createEventDispatcher } from 'svelte';

	export let viewport: HTMLDivElement | undefined = undefined;
	export let contents: HTMLDivElement | undefined = undefined;
	export let height: string | undefined = undefined;
	export let minHeight: string | undefined = undefined;
	export let scrollable: boolean | undefined = undefined;
	export let maxHeight: number | undefined = undefined;
	export let scrolled = false;
	export let wide = false;
	export let initiallyVisible = false;
	export let showBorderWhenScrolled = false;

	let observer: ResizeObserver;

	const dispatch = createEventDispatcher<{ dragging: boolean }>();

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

<div
	class="scrollable"
	class:scrolled={showBorderWhenScrolled && scrolled}
	style:flex-grow={wide ? 1 : 0}
	style:min-height={minHeight}
>
	<div
		bind:this={viewport}
		on:scroll={(e) => {
			scrolled = e.currentTarget.scrollTop != 0;
		}}
		class="viewport hide-native-scrollbar"
		style:height
		style:overflow-y={scrollable ? 'auto' : 'hidden'}
	>
		<div bind:this={contents} class="contents">
			<slot />
		</div>
		<Scrollbar
			{viewport}
			{contents}
			{initiallyVisible}
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
		border-top: 1px solid var(--clr-theme-container-outline-light);
	}
</style>

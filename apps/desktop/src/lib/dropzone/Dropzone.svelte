<script lang="ts">
	import { dropzone } from '$lib/dragging/dropzone';
	import type { Snippet } from 'svelte';

	interface Props {
		disabled?: boolean;
		fillHeight?: boolean;
		accepts: (data: any) => boolean;
		ondrop: (data: any) => Promise<void> | void;
		overlay?: Snippet<[{ hovered: boolean; activated: boolean }]>;
		children?: Snippet;
	}

	const {
		disabled = false,
		fillHeight = false,
		accepts,
		ondrop,
		overlay,
		children
	}: Props = $props();

	let hovered = $state(false);
	// When a draggable is being hovered over the dropzone
	function onHoverStart() {
		hovered = true;
	}

	function onHoverEnd() {
		hovered = false;
	}

	let activated = $state(false);
	// Fired when a draggable is first picked up and the dropzone accepts the draggable
	function onActivationStart() {
		activated = true;
	}

	function onActivationEnd() {
		activated = false;
	}
</script>

<div
	use:dropzone={{
		disabled,
		accepts,
		onDrop: ondrop,
		onHoverStart,
		onHoverEnd,
		onActivationStart,
		onActivationEnd,
		target: '.dropzone-target'
	}}
	class:fill-height={fillHeight}
	class="dropzone-container"
>
	{#if overlay}
		{@render overlay({ hovered, activated })}
	{/if}

	{#if children}
		{@render children()}
	{/if}
</div>

<style lang="postcss">
	.fill-height {
		display: flex;
		flex-direction: column;
		flex-grow: 1;
	}

	.dropzone-container {
		position: relative;
	}
</style>

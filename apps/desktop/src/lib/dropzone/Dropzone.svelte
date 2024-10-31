<script lang="ts">
	import { dropzone } from '$lib/dragging/dropzone';
	import type { Snippet } from 'svelte';

	interface Props {
		disabled?: boolean;
		fillHeight?: boolean;
		accepts: (data: any) => boolean;
		ondrop: (data: any) => Promise<void> | void;
		overlay: Snippet<[{ hovered: boolean; activated: boolean }]>;
		stackingReorderLine?: boolean;
		children?: Snippet;
	}

	const {
		disabled = false,
		fillHeight = false,
		accepts,
		ondrop,
		overlay,
		stackingReorderLine = false,
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
		stackingReorderLine,
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
	{@render overlay({ hovered, activated })}

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

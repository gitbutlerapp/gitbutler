<script lang="ts">
	import { dropzone, type HoverArgs } from '$lib/dragging/dropzone';
	import type { DropzoneHandler } from '$lib/dragging/handler';
	import type { Snippet } from 'svelte';

	interface Props {
		disabled?: boolean;
		fillHeight?: boolean;
		handlers: DropzoneHandler[];
		overlay?: Snippet<[{ hovered: boolean; activated: boolean; handler?: DropzoneHandler }]>;
		children?: Snippet;
	}

	const { disabled = false, fillHeight = false, handlers, overlay, children }: Props = $props();

	let hovered = $state(false);
	let hoveredHandler: DropzoneHandler | undefined = $state();

	// When a draggable is being hovered over the dropzone
	function onHoverStart(args: HoverArgs) {
		hovered = true;
		hoveredHandler = args.handler;
	}

	function onHoverEnd() {
		hovered = false;
		hoveredHandler = undefined;
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
		handlers,
		disabled,
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
		{@render overlay({ hovered, activated, handler: hoveredHandler })}
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
		display: flex;
		flex-direction: column;
		position: relative;
		flex-grow: 1;
		width: 100%;
	}
</style>

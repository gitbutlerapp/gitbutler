<script lang="ts">
	import { dropzone, type HoverArgs } from '$lib/dragging/dropzone';
	import { DROPZONE_REGISTRY } from '$lib/dragging/registry';
	import { inject } from '@gitbutler/core/context';
	import type { DropzoneHandler } from '$lib/dragging/handler';
	import type { Snippet } from 'svelte';

	const dropzoneRegistry = inject(DROPZONE_REGISTRY);

	interface Props {
		disabled?: boolean;
		fillHeight?: boolean;
		maxHeight?: boolean;
		handlers: DropzoneHandler[];
		hideWhenInactive?: boolean;
		onActivated?: (activated: boolean) => void;
		overlay?: Snippet<[{ hovered: boolean; activated: boolean; handler?: DropzoneHandler }]>;
		children?: Snippet;
		overflow?: boolean;
	}

	const {
		disabled = false,
		fillHeight = false,
		maxHeight = false,
		handlers,
		onActivated,
		overlay,
		children,
		hideWhenInactive,
		overflow
	}: Props = $props();

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
		onActivated?.(activated);
	}

	function onActivationEnd() {
		activated = false;
		onActivated?.(activated);
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
		target: '.dropzone-target',
		registry: dropzoneRegistry
	}}
	class:fill-height={fillHeight}
	class:max-height={maxHeight}
	style:display={hideWhenInactive && !activated ? 'none' : undefined}
	style:overflow={overflow ? 'hidden' : undefined}
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
		flex-grow: 1;
		flex-direction: column;
	}

	.max-height {
		height: 100%;
	}

	.dropzone-container {
		display: flex;
		position: relative;
		flex-grow: 1;
		flex-direction: column;
		width: 100%;
	}
</style>

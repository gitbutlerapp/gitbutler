<script lang="ts">
	import { floatingDiv, type FloatingDivOptions } from '$lib/dragging/float';
	import { persistWithExpiration } from '@gitbutler/shared/persisted';
	import { onMount, type Snippet } from 'svelte';

	type Props = {
		persistId: string;
		initialPosition: { x: number; y: number };
		initialSize: { width: number; height: number };
		children: Snippet<[]>;
	};

	const { initialPosition, initialSize, persistId, children }: Props = $props();

	let persistedPosition = persistWithExpiration(
		{ x: initialPosition.x, y: initialPosition.y },
		`irc-float-position-${persistId}`,
		24 * 60
	);

	let persistedSize = persistWithExpiration(
		{ width: initialSize.width, height: initialSize.height },
		`irc-float-size-${persistId}`,
		24 * 60
	);

	let element = $state<HTMLDivElement>();

	// Floating window settings
	let floatOptions: FloatingDivOptions = {
		initialPosition: $persistedPosition,
		initialSize: $persistedSize,
		handleSelector: '.irc-channel .header',
		zIndex: 100,
		resizeHandles: { right: true, bottom: true, corner: true },
		minWidth: 300,
		minHeight: 200,
		maxWidth: 600,
		maxHeight: 800,
		onDragEnd: (event) => persistedPosition.set(event.position),
		onResizeEnd: (event) => persistedSize.set(event.size)
	};

	function onIntersects() {
		if (element) {
			if (element.offsetLeft + element.offsetWidth > window.innerWidth) {
				element.style.left = window.innerWidth - element.offsetWidth - 50 + 'px';
			}
			if (element.offsetTop + element.offsetHeight > window.innerHeight) {
				element.style.top = window.innerHeight - element.offsetHeight - 50 + 'px';
			}
		}
	}

	onMount(() => {
		if (element) {
			const intersectsObserver = new IntersectionObserver(onIntersects, { threshold: 1 });
			intersectsObserver.observe(element);

			return () => {
				intersectsObserver.disconnect();
			};
		}
	});
</script>

<div bind:this={element} class="irc-float" use:floatingDiv={floatOptions}>
	{@render children()}
</div>

<style lang="postcss">
	.irc-float {
		background-color: var(--clr-bg-1);
		position: absolute;
		z-index: var(--z-lifted);
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
		overflow: hidden;
		box-shadow: var(--fx-shadow-m);
	}
</style>

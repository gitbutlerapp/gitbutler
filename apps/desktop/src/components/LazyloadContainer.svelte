<script lang="ts">
	import { onMount } from 'svelte';
	import type { AriaRole } from 'svelte/elements';

	interface Props {
		children: any;
		minTriggerCount: number;
		role?: AriaRole | undefined | null;
		ontrigger: (lastChild: Element) => void;
		onkeydown?: (e: KeyboardEvent) => void;
		/**
		 * Number of items currently rendered. When provided, the component uses this
		 * instead of querying the DOM for child count, avoiding expensive operations
		 * with `display: contents`.
		 */
		itemCount?: number;
	}

	const { children, minTriggerCount, role, ontrigger, onkeydown, itemCount }: Props = $props();

	let sentinelEl = $state<HTMLDivElement>();
	let lazyContainerEl = $state<HTMLDivElement>();

	const intersectionObserver = new IntersectionObserver((entries) => {
		entries.forEach((entry) => {
			if (entry.isIntersecting) {
				ontrigger(entry.target);
				// Unobserve to prevent multiple triggers for the same intersection
				// The $effect will re-observe the sentinel when itemCount changes
				intersectionObserver.unobserve(entry.target);
			}
		});
	});

	// When itemCount is provided, use it directly (fast path)
	// Otherwise, fall back to MutationObserver (legacy behavior)
	$effect(() => {
		if (itemCount !== undefined) {
			// Fast path: use prop-based count
			intersectionObserver.disconnect();
			if (sentinelEl && itemCount >= minTriggerCount) {
				intersectionObserver.observe(sentinelEl);
			}
		} else if (lazyContainerEl) {
			// Legacy path: use MutationObserver
			mutuationObserver.disconnect();
			mutuationObserver.observe(lazyContainerEl, { childList: true });
			attachIntersectionObserver();
		}
	});

	const mutuationObserver = new MutationObserver(attachIntersectionObserver);

	function attachIntersectionObserver() {
		// unattach all intersection observers
		intersectionObserver.disconnect();
		if (!lazyContainerEl) return;

		const containerChildren = lazyContainerEl.children;
		if (containerChildren.length < minTriggerCount) return;

		const lastChild = containerChildren[containerChildren.length - 1];
		if (!lastChild) return;

		intersectionObserver.observe(lastChild);
	}

	onMount(() => {
		return () => {
			intersectionObserver.disconnect();
			mutuationObserver.disconnect();
		};
	});

	export function hasFocus() {
		return (
			document.activeElement === lazyContainerEl ||
			lazyContainerEl?.contains(document.activeElement)
		);
	}
</script>

<div class="lazy-container" {role} bind:this={lazyContainerEl} {onkeydown}>
	{@render children()}
	{#if itemCount !== undefined}
		<div class="lazy-sentinel" bind:this={sentinelEl}></div>
	{/if}
</div>

<style>
	.lazy-container {
		display: contents;
	}

	.lazy-sentinel {
		height: 1px;
		width: 100%;
	}
</style>

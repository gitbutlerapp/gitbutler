<script lang="ts">
	import { onMount } from 'svelte';
	import { intersectionObserver } from '$lib/utils/intersectionObserver';

	interface Props {
		children: any;
		minTriggerCount?: number;
		ontrigger: (lastChild: Element) => void;
	}

	let { children, minTriggerCount = 40, ontrigger }: Props = $props();

	let lazyContainerEl: HTMLDivElement;

	onMount(() => {
		const containerChildren = lazyContainerEl.children;

		if (containerChildren.length > minTriggerCount) return;

		const lastChild = containerChildren[containerChildren.length - 1];

		intersectionObserver(lastChild, {
			isDisabled: false,
			callback: (entry) => {
				if (entry.isIntersecting) {
					ontrigger(lastChild);
				}
			},
			options: { threshold: 0 }
		});
	});
</script>

<div class="lazy-container" bind:this={lazyContainerEl}>
	{@render children()}
</div>

<style>
	.lazy-container {
		display: contents;
	}
</style>

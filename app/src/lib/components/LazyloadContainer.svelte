<script lang="ts">
	import { intersectionObserver } from '$lib/utils/intersectionObserver';

	interface Props {
		children: any;
		ontrigger: (lastChild: HTMLElement) => void;
	}

	let { children, ontrigger }: Props = $props();

	let lazeContainerEl: HTMLDivElement;

	$effect(() => {
		const containerChildren = lazeContainerEl.children;
		const lastChild = containerChildren[containerChildren.length - 1] as HTMLElement;

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

<div class="lazy-container" bind:this={lazeContainerEl}>
	{@render children()}
</div>

<style>
	.lazy-container {
		display: contents;
	}
</style>

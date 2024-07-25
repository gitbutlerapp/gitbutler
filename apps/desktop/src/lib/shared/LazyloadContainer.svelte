<script lang="ts">
	import { onMount } from 'svelte';
	import type { AriaRole } from 'svelte/elements';

	interface Props {
		children: any;
		minTriggerCount: number;
		role?: AriaRole | undefined | null;
		ontrigger: (lastChild: Element) => void;
	}

	let { children, minTriggerCount, role, ontrigger }: Props = $props();

	let lazyContainerEl: HTMLDivElement;

	onMount(() => {
		const containerChildren = lazyContainerEl.children;

		if (containerChildren.length < minTriggerCount) return;

		const iObserver = new IntersectionObserver((entries) => {
			const lastChild = containerChildren[containerChildren.length - 1];
			if (entries[0].target === lastChild && entries[0].isIntersecting) {
				ontrigger(lastChild);
			}
		});

		const mObserver = new MutationObserver(() => {
			const lastChild = containerChildren[containerChildren.length - 1];
			if (lastChild) {
				iObserver.observe(lastChild);
			}
		});

		iObserver.observe(containerChildren[containerChildren.length - 1]);
		mObserver.observe(lazyContainerEl, { childList: true });

		return () => {
			iObserver.disconnect();
			mObserver.disconnect();
		};
	});
</script>

<div class="lazy-container" {role} bind:this={lazyContainerEl}>
	{@render children()}
</div>

<style>
	.lazy-container {
		display: contents;
	}
</style>

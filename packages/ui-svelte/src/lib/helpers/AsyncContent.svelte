<!--
	A helper component for testing the VirtuaList. It would be good to find a
	better location for this file, something accessible to both tests as well
	as stories.
-->
<script lang="ts">
	import type { Snippet } from "svelte";

	type Props = {
		delay?: number;
		children: Snippet;
	};

	const { delay, children }: Props = $props();

	let visible = $state(false);

	$effect(() => {
		if (delay) {
			setTimeout(() => {
				visible = true;
			}, delay);
		} else {
			visible = false;
		}
	});
</script>

{#if !delay || (delay && visible)}
	{@render children()}
{/if}

<style lang="postcss">
</style>

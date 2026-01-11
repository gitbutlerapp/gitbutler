<script lang="ts">
	import type { Snippet } from 'svelte';

	type Props = {
		delay: number;
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

{#if delay === undefined || (delay && visible)}
	{@render children()}
{/if}

<style lang="postcss">
</style>

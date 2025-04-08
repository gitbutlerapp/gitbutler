<script lang="ts">
	import type { Snippet } from 'svelte';

	type Props = {
		delay: number;
		children: Snippet;
	};

	const { delay, children }: Props = $props();

	let hide = $state(true);

	$effect(() => {
		if (hide && delay !== 0) {
			setTimeout(() => {
				hide = false;
			}, delay);
		}
	});
</script>

{#if !hide || delay === 0}
	{#if children}
		{@render children()}
	{/if}
{/if}

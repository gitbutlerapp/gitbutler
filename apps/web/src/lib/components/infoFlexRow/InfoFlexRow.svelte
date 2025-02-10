<script lang="ts">
	import type { Snippet } from 'svelte';

	type Props = {
		children: Snippet;
	};

	const { children }: Props = $props();
</script>

<div class="info-flex-row">
	{#if children}
		{@render children()}
	{/if}
</div>

<style lang="postcss">
	.info-flex-row {
		display: flex;
		flex-wrap: wrap;
		gap: 16px 30px;
	}
</style>

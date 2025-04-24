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
		container: info-flex-row;

		/* TODO: Add contaner query here. It doesn't work in the current version of Svelte. */
		/* display: grid;
		grid-template-columns: repeat(auto-fill, minmax(74px, 1fr));
		grid-auto-flow: row dense;
		grid-gap: 16px 30px; */
	}
</style>

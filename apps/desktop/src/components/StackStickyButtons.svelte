<script lang="ts">
	import type { Snippet } from 'svelte';

	type Props = {
		children: Snippet<[boolean]>;
	};

	let { children }: Props = $props();
	let isSticked = $state(true);
</script>

<div class="sticky-buttons" class:is-sticked={isSticked}>
	{#if children}
		{@render children(isSticked)}
	{/if}
</div>

<style lang="postcss">
	.sticky-buttons {
		position: sticky;
		bottom: 0;
		z-index: var(--z-lifted);
		padding: 12px 12px 14px 12px;
		/* margin: 0 -1px; */
		display: flex;
		gap: 6px;
		background-color: var(--clr-bg-1);
		border-top: 1px solid var(--clr-border-2);
	}
</style>

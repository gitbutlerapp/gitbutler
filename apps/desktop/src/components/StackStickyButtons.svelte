<script lang="ts">
	import type { Snippet } from 'svelte';

	type Props = {
		children: Snippet<[boolean]>;
	};

	let { children }: Props = $props();
	let isSticked = $state(true);
</script>

<div class="sticky-buttons" data-remove-from-draggable class:is-sticked={isSticked}>
	{#if children}
		{@render children(isSticked)}
	{/if}
</div>

<style lang="postcss">
	.sticky-buttons {
		display: flex;
		padding: 12px 12px 14px 12px;
		gap: 6px;
		border-top: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-1);
	}
</style>

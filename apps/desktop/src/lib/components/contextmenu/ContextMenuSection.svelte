<script lang="ts">
	import type { Snippet } from 'svelte';

	const { title, children }: { title?: string; children: Snippet } = $props();
</script>

<div class="context-menu-section">
	{#if title}
		<div class="context-menu-section__title text-11 text-semibold">{title}</div>
	{/if}
	{@render children()}
</div>

<style lang="postcss">
	.context-menu-section {
		position: relative;
		display: flex;
		flex-direction: column;
		padding: 4px;
		gap: 2px;

		&:not(:first-child) {
			border-top: 1px solid var(--clr-border-2);
		}
	}

	.context-menu-section__title {
		padding: 8px 8px 6px;
		color: var(--clr-text-3);
		user-select: none;
	}
</style>

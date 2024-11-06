<script lang="ts">
	import type { Snippet } from 'svelte';

	const { label, children }: { label?: string; children: Snippet } = $props();
</script>

<div class="context-menu-section">
	{#if label}
		<div class="label text-12">{label}</div>
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

	.label {
		padding: 6px 8px;
		color: var(--clr-scale-ntrl-50);
		user-select: none;
	}
</style>

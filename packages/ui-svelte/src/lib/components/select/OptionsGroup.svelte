<script lang="ts">
	import { type Snippet } from "svelte";

	interface Props {
		header?: string;
		children: Snippet;
	}

	const { header, children }: Props = $props();
</script>

<div class="options__group">
	{#if header}
		<div class="options__group-header text-11 text-semibold">{header}</div>
	{/if}
	{@render children()}
</div>

<style lang="postcss">
	.options__group {
		display: flex;
		flex-direction: column;
		padding: 6px;
		gap: 2px;

		&:not(&:last-child) {
			border-bottom: 1px solid var(--border-2);
		}
	}

	.options__group-header {
		padding: 8px 8px 2px;
		color: var(--text-3);
		letter-spacing: 0.04em;
		text-transform: uppercase;
		user-select: none;
	}
</style>

<script lang="ts">
	import Badge from '$lib/shared/Badge.svelte';
	import type { Snippet } from 'svelte';

	interface Props {
		filteredBranchCount?: number;
		totalBranchCount: number;
		filtersActive: boolean;
		filterButton: Snippet<[filtersActive: boolean]>;
	}

	const { filteredBranchCount, totalBranchCount, filtersActive, filterButton }: Props = $props();
</script>

<div class="header">
	<div class="branches-title">
		<span class="text-base-14 text-bold">Branches</span>

		{#if filteredBranchCount !== undefined}
			<Badge count={filteredBranchCount} />
		{/if}
	</div>
	{#if totalBranchCount > 0}
		{@render filterButton(filtersActive)}
	{/if}
</div>

<style lang="postcss">
	.header {
		display: flex;
		color: var(--clr-scale-ntrl-0);
		align-items: center;
		justify-content: space-between;
		width: 100%;
		padding: 14px 14px 12px 14px;
		gap: 8px;
		border-bottom: 1px solid transparent;
		transition: border-bottom var(--transition-fast);
		position: relative;
	}
	.branches-title {
		display: flex;
		align-items: center;
		gap: 4px;
	}
</style>

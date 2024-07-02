<script lang="ts">
	import { clickOutside } from '$lib/clickOutside';
	import Badge from '$lib/shared/Badge.svelte';
	import Button from '$lib/shared/Button.svelte';
	import type { Snippet } from 'svelte';

	interface Props {
		filteredBranchCount?: number;
		totalBranchCount: number;
		filtersActive: boolean;
		contextMenu: Snippet<[{ visible: boolean }]>;
	}

	const { filteredBranchCount, totalBranchCount, filtersActive, contextMenu }: Props = $props();

	let visible = $state(false);
	let filterButton = $state<HTMLDivElement>();

	function onFilterClick(e: Event) {
		visible = !visible;
		e.preventDefault();
		e.stopPropagation();
	}
</script>

<div class="header">
	<div class="branches-title">
		<span class="text-base-14 text-bold">Branches</span>

		{#if filteredBranchCount !== undefined}
			<Badge count={filteredBranchCount} />
		{/if}
	</div>
	{#if totalBranchCount > 0}
		<div class="header__filter-btn" bind:this={filterButton}>
			<Button
				style="ghost"
				outline
				icon={filtersActive ? 'filter-applied-small' : 'filter-small'}
				on:mousedown={onFilterClick}
			>
				Filter
			</Button>
			<div
				class="filter-popup-menu"
				use:clickOutside={{ trigger: filterButton, handler: () => (visible = false) }}
			>
				{@render contextMenu({ visible })}
			</div>
		</div>
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
	.header__filter-btn {
		position: relative;
	}
	.filter-popup-menu {
		position: absolute;
		top: calc(var(--size-button) + 4px);
		right: 0;
		z-index: var(--z-floating);
		min-width: 160px;
	}
	.branches-title {
		display: flex;
		align-items: center;
		gap: 4px;
	}
</style>

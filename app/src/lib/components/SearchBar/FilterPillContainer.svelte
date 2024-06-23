<script lang="ts">
	import {
		formatFilterName,
		formatFilterValues,
		type AppliedFilter
	} from '$lib/vbranches/filtering';
	import FilterPill from './FilterPill.svelte';

	interface Props {
		appliedFilters: AppliedFilter[];
		handleFilterClick: (filter: AppliedFilter) => void;
	}
	let { appliedFilters, handleFilterClick }: Props = $props();
</script>

<div class="filter-pill-container text-base-14">
	{#each appliedFilters as filter}
		<FilterPill
			name={formatFilterName(filter)}
			value={formatFilterValues(filter)}
			onclick={() => handleFilterClick(filter)}
		/>
	{/each}
</div>

<style>
	.filter-pill-container {
		margin-right: 10px;
		display: flex;
		gap: 4px;
	}
</style>

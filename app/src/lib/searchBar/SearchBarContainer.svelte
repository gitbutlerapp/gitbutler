<script lang="ts">
	import SearchBar from './SearchBar.svelte';
	import { FilterName, type AppliedFilter, type FilterDescription } from '$lib/vbranches/filtering';
	import type { Snippet } from 'svelte';

	interface Props {
		searchQuery: string | undefined;
		searchFilters: AppliedFilter[];
		filterDescriptions: FilterDescription[];
		children: Snippet;
	}

	let {
		searchQuery = $bindable(),
		searchFilters = $bindable(),
		filterDescriptions,
		children
	}: Props = $props();

	let searchBarElem = $state<SearchBar | undefined>(undefined);

	export function addAuthorFilter(author: string) {
		searchBarElem?.applyFilter({ name: FilterName.Author }, [author]);
	}

	export function addFileFilter(filePath: string) {
		searchBarElem?.applyFilter({ name: FilterName.File }, [filePath]);
	}
</script>

<div class="container">
	<div class="search">
		<SearchBar
			bind:this={searchBarElem}
			bind:value={searchQuery}
			bind:appliedFilters={searchFilters}
			{filterDescriptions}
			icon="search"
			placeholder="Search"
		/>
	</div>
	{#if children}
		{@render children()}
	{/if}
</div>

<style>
	.container {
		display: flex;
		flex-direction: column;
		flex-grow: 1;
		overflow: hidden;
	}

	.search {
		padding: 12px;
		padding-bottom: 0;
	}
</style>

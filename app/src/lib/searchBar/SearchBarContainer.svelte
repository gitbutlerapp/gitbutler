<script lang="ts">
	import SearchBar from './SearchBar.svelte';
	import { type FilterDescription } from '$lib/vbranches/filtering';
	import { type Snippet } from 'svelte';

	interface Props {
		filterDescriptions: FilterDescription[];
		children: Snippet;
	}

	let { filterDescriptions, children }: Props = $props();

	let searchBarElem = $state<SearchBar | undefined>(undefined);
</script>

<div class="container">
	<div class="search">
		<SearchBar bind:this={searchBarElem} {filterDescriptions} icon="search" placeholder="Search" />
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

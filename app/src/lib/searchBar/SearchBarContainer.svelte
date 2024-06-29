<script lang="ts">
	import SearchBar from './SearchBar.svelte';
	import { type FilterDescription } from '$lib/vbranches/filtering';
	import { type Snippet } from 'svelte';

	interface Props {
		filterDescriptions: FilterDescription[];
		onFocus?: () => void;
		children: Snippet;
	}

	let { filterDescriptions, onFocus, children }: Props = $props();

	let searchBarElem = $state<SearchBar | undefined>(undefined);
</script>

<div class="container">
	<div class="search">
		<SearchBar
			bind:this={searchBarElem}
			{filterDescriptions}
			{onFocus}
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
		box-sizing: border-box;
	}

	.search {
		padding: 12px;
		padding-bottom: 0;
		box-sizing: border-box;
	}
</style>

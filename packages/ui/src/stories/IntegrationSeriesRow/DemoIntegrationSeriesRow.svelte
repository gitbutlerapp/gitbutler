<script lang="ts">
	import IntegrationSeriesListItem, {
		type Props as SeriesProps
	} from '$lib/IntegrationSeriesRow.svelte';

	const items = [
		{ type: 'clear', title: 'Move context into shared' },
		{ type: 'conflicted', title: 'Add new context' },
		{ type: 'integrated', title: 'Make edit mode fantastic' }
	] as SeriesProps[];
</script>

<div class="wrapper">
	{#each items as item}
		<IntegrationSeriesListItem {...item} />
	{/each}
</div>

<style>
	.wrapper {
		display: flex;
		flex-direction: column;
		border-radius: var(--radius-m);
		border: 1px solid var(--clr-border-2);
		overflow: hidden;
	}
</style>

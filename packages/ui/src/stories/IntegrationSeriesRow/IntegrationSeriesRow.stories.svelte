<script module lang="ts">
	import IntegrationSeriesListItem, {
		type Props as SeriesProps
	} from '$lib/IntegrationSeriesRow.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	const items = [
		{
			series: [
				{ name: 'feature/add-user-auth', status: 'integrated' },
				{ name: 'feature/update-user-profile' },
				{ name: 'feature/create-payment-gateway', status: 'conflicted' },
				{ name: 'feature/improve-dashboard', status: 'integrated' },
				{ name: 'feature/add-user-notifications' }
			]
		},
		{ series: [{ name: 'feature/add-user-auth', status: 'integrated' }] },
		{
			series: [
				{ name: 'feature/add-user-auth', status: 'integrated' },
				{ name: 'feature/update-user-profile' },
				{ name: 'feature/create-payment-gateway', status: 'conflicted' },
				{ name: 'feature/improve-dashboard', status: 'integrated' },
				{ name: 'feature/add-user-notifications' },
				{ name: 'feature/optimize-search', status: 'conflicted' },
				{ name: 'feature/refactor-api', status: 'integrated' },
				{ name: 'feature/fix-bug-123' },
				{ name: 'feature/add-analytics-tracking', status: 'conflicted' },
				{ name: 'feature/add-multi-language-support', status: 'integrated' }
			]
		}
	] as SeriesProps[];

	const { Story } = defineMeta({
		title: 'List items / Integration Series Row'
	});
</script>

<Story name="Variants">
	{#each items as item}
		<IntegrationSeriesListItem {...item} />
	{/each}
</Story>

<script module lang="ts">
	import IntegrationSeriesListItem, {
		type Props as SeriesProps
	} from '$components/IntegrationSeriesRow.svelte';
	import { defineMeta } from '@storybook/addon-svelte-csf';

	// eslint-disable-next-line func-style
	const updateBranchShouldBeDeletedMap = (branchName: string[], shouldBeDeleted: boolean) => {
		// Mock function to update the branch deletion status
		// eslint-disable-next-line no-console
		console.log(`Branches ${branchName.join(',')} should be deleted: ${shouldBeDeleted}`);
	};
	const items: SeriesProps[] = [
		{
			series: [
				{ name: 'feature/add-user-auth', status: 'integrated' },
				{ name: 'feature/update-user-profile', status: 'clear' },
				{ name: 'feature/create-payment-gateway', status: 'conflicted' },
				{ name: 'feature/improve-dashboard', status: 'integrated' },
				{ name: 'feature/add-user-notifications', status: 'clear' }
			],
			branchShouldBeDeletedMap: {
				'feature/add-user-auth': true,
				'feature/update-user-profile': false,
				'feature/create-payment-gateway': false,
				'feature/improve-dashboard': true,
				'feature/add-user-notifications': false
			},
			updateBranchShouldBeDeletedMap
		},
		{
			series: [{ name: 'feature/add-user-auth', status: 'integrated' }],
			branchShouldBeDeletedMap: {
				'feature/add-user-auth': true
			},
			updateBranchShouldBeDeletedMap
		},
		{
			series: [
				{ name: 'feature/add-user-auth', status: 'integrated' },
				{ name: 'feature/update-user-profile', status: 'clear' },
				{ name: 'feature/create-payment-gateway', status: 'conflicted' },
				{ name: 'feature/improve-dashboard', status: 'integrated' },
				{ name: 'feature/add-user-notifications', status: 'clear' },
				{ name: 'feature/optimize-search', status: 'conflicted' },
				{ name: 'feature/refactor-api', status: 'integrated' },
				{ name: 'feature/fix-bug-123', status: 'clear' },
				{ name: 'feature/add-analytics-tracking', status: 'integrated' },
				{ name: 'feature/add-multi-language-support', status: 'conflicted' },
				{ name: 'feature/add-analytics-tracking', status: 'conflicted' },
				{ name: 'feature/add-multi-language-support', status: 'integrated' }
			],
			branchShouldBeDeletedMap: {
				'feature/add-user-auth': true,
				'feature/update-user-profile': false,
				'feature/create-payment-gateway': false,
				'feature/improve-dashboard': true,
				'feature/add-user-notifications': false,
				'feature/optimize-search': false,
				'feature/refactor-api': true,
				'feature/fix-bug-123': false,
				'feature/add-analytics-tracking': false,
				'feature/add-multi-language-support': true
			},
			updateBranchShouldBeDeletedMap
		}
	];

	const { Story } = defineMeta({
		title: 'List items / Integration Series Row'
	});
</script>

<Story name="Variants">
	{#each items as item}
		<IntegrationSeriesListItem {...item} />
	{/each}
</Story>

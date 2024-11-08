<script lang="ts">
	import IntegrationSeriesListItem, {
		type Props as SeriesProps
	} from '$lib/IntegrationSeriesRow.svelte';

	const items = [
		{
			type: 'clear',
			series: [
				'feature/improve-dashboard-ui',
				'release/v1.2.0',
				'feature/refactor-api-endpoints',
				'bugfix/remove-duplicate-entries',
				'chore/update-dependencies',
				'feature/add-password-reset'
			]
		},
		{ type: 'conflicted', series: ['feature/add-user-auth'] },
		{
			type: 'integrated',
			series: [
				'feature/add-user-auth',
				'bugfix/fix-login-error',
				'hotfix/update-ssl-cert',
				'feature/improve-dashboard-ui',
				'release/v1.2.0',
				'feature/refactor-api-endpoints',
				'bugfix/remove-duplicate-entries',
				'chore/update-dependencies',
				'feature/add-password-reset'
			]
		}
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

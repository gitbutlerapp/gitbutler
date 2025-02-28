<script lang="ts">
	import BranchIndexCard from '$lib/components/branches/BranchIndexCard.svelte';
	import Table from '$lib/components/table/Table.svelte';
	import { BranchService } from '@gitbutler/shared/branches/branchService';
	import { getBranchReviewsForRepository } from '@gitbutler/shared/branches/branchesPreview.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import { type ProjectParameters } from '@gitbutler/shared/routing/webRoutes.svelte';
	import Badge from '@gitbutler/ui/Badge.svelte';

	interface Props {
		data: ProjectParameters;
	}

	let { data }: Props = $props();

	const branchService = getContext(BranchService);
	const appState = getContext(AppState);

	const brancheses = $derived(
		getBranchReviewsForRepository(appState, branchService, data.ownerSlug, data.projectSlug)
	);
</script>

<svelte:head>
	<title>Review: {data.ownerSlug}/{data.projectSlug}</title>
</svelte:head>

<Loading loadable={brancheses?.current}>
	{#snippet children(brancheses)}
		<div class="title">
			<div class="text-16 text-bold">Branches shared for review</div>
			<Badge>{brancheses.length || 0}</Badge>
		</div>

		<Table
			headColumns={[
				{
					key: 'status',
					value: 'Status'
				},
				{
					key: 'title',
					value: 'Name'
				},
				{
					key: 'number',
					value: 'UUID'
				},
				{
					key: 'number',
					value: 'Commits'
				},
				{
					key: 'date',
					value: 'Update'
				},
				{
					key: 'avatars',
					value: 'Authors'
				},
				{
					key: 'number',
					value: 'Ver.',
					tooltip: 'Commit version'
				}
			]}
		>
			{#snippet body()}
				{#each brancheses as branches, i}
					{#each branches as branch, j}
						<BranchIndexCard
							linkParams={data}
							uuid={branch.uuid}
							roundedTop={j === 0 && i !== 0}
							roundedBottom={j === branches.length - 1}
						/>
					{/each}
				{/each}
			{/snippet}
		</Table>
	{/snippet}
</Loading>

<style>
	.title {
		display: flex;
		align-items: center;
		gap: 6px;
		margin-bottom: 24px;
	}
</style>

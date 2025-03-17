<script lang="ts">
	import BranchIndexCard from '$lib/components/branches/BranchIndexCard.svelte';
	import DashboardLayout from '$lib/components/dashboard/DashboardLayout.svelte';
	import Table from '$lib/components/table/Table.svelte';
	import { getBranchReviewsForRepository } from '@gitbutler/shared/branches/branchesPreview.svelte';
	import { BranchStatus } from '@gitbutler/shared/branches/types';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { type ProjectParameters } from '@gitbutler/shared/routing/webRoutes.svelte';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import Select from '@gitbutler/ui/select/Select.svelte';
	import SelectItem from '@gitbutler/ui/select/SelectItem.svelte';

	interface Props {
		data: ProjectParameters;
	}

	let { data }: Props = $props();

	let filterStatus = $state<BranchStatus>(BranchStatus.All);
	const selectableStatuses = [
		{ value: BranchStatus.All, label: 'All' },
		{ value: BranchStatus.Closed, label: 'Closed' },
		{ value: BranchStatus.Active, label: 'Active' },
		{ value: BranchStatus.Inactive, label: 'Inactive' }
	];

	const brancheses = $derived(
		getBranchReviewsForRepository(data.ownerSlug, data.projectSlug, filterStatus)
	);
</script>

<svelte:head>
	<title>Review: {data.ownerSlug}/{data.projectSlug}</title>
</svelte:head>

{#snippet filters()}
	<Select
		options={selectableStatuses}
		value={filterStatus}
		autoWidth
		onselect={(value) => {
			filterStatus = value as BranchStatus;
		}}
	>
		{#snippet customSelectButton()}
			<Button kind="ghost" icon="chevron-down" size="tag">
				{selectableStatuses.find((status) => status.value === filterStatus)!.label}
			</Button>
		{/snippet}
		{#snippet itemSnippet({ item, highlighted })}
			<SelectItem {highlighted}>{item.label}</SelectItem>
		{/snippet}
	</Select>
{/snippet}

<DashboardLayout>
	<Loading loadable={brancheses?.current}>
		{#snippet children(brancheses)}
			<div class="header">
				<div class="title">
					<div class="text-16 text-bold">Branches shared for review</div>
					<Badge>{brancheses.length || 0}</Badge>
				</div>
				{@render filters()}
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
</DashboardLayout>

<style>
	.header {
		display: flex;
		align-items: center;

		justify-content: space-between;

		margin-bottom: 24px;
	}
	.title {
		display: flex;
		align-items: center;
		gap: 6px;
	}
</style>

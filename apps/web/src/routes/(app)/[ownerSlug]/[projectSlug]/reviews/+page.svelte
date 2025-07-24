<script lang="ts">
	import { goto } from '$app/navigation';
	import { AUTH_SERVICE } from '$lib/auth/authService.svelte';
	import BranchIndexCard from '$lib/components/branches/BranchIndexCard.svelte';
	import DashboardLayout from '$lib/components/dashboard/DashboardLayout.svelte';
	import Table from '$lib/components/table/Table.svelte';
	import { getBranchReviewsForRepository } from '@gitbutler/shared/branches/branchesPreview.svelte';
	import { BranchStatus } from '@gitbutler/shared/branches/types';
	import { inject } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { getProject } from '@gitbutler/shared/organizations/projectsPreview.svelte';
	import { type ProjectParameters } from '@gitbutler/shared/routing/webRoutes.svelte';
	import { WEB_ROUTES_SERVICE } from '@gitbutler/shared/routing/webRoutes.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import Select from '@gitbutler/ui/select/Select.svelte';
	import SelectItem from '@gitbutler/ui/select/SelectItem.svelte';

	// Get authentication service and check if user is logged in
	const authService = inject(AUTH_SERVICE);
	const routes = inject(WEB_ROUTES_SERVICE);

	// If there is no token (user not logged in), redirect to home
	$effect(() => {
		if (!authService.token.current) {
			goto(routes.homePath());
		}
	});

	interface Props {
		data: ProjectParameters;
	}

	let { data }: Props = $props();

	let filterStatus = $state<BranchStatus>(BranchStatus.All);
	const selectableStatuses = [
		{ value: BranchStatus.All, label: 'All branches' },
		{ value: BranchStatus.Closed, label: 'Closed' },
		{ value: BranchStatus.Active, label: 'Active' },
		{ value: BranchStatus.Inactive, label: 'Inactive' }
	];

	const brancheses = $derived(
		getBranchReviewsForRepository(data.ownerSlug, data.projectSlug, filterStatus)
	);

	const project = $derived(getProject(data.ownerSlug, data.projectSlug));
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
			<Button kind="ghost" icon="chevron-down">
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
					<Loading loadable={project.current}>
						{#snippet children(project)}
							<div class="text-16 text-bold">{project.name}</div>
						{/snippet}
					</Loading>
				</div>
				{@render filters()}
			</div>

			{#if brancheses.length === 0}
				<div class="empty-state">
					<h3>No branches found</h3>
					<p>There are no branches matching your current filter.</p>
				</div>
			{:else}
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
									isTopEntry={i + j === 0}
									roundedTop={j === 0 && i !== 0}
									roundedBottom={j === branches.length - 1}
								/>
							{/each}
						{/each}
					{/snippet}
				</Table>
			{/if}
		{/snippet}
	</Loading>
</DashboardLayout>

<style>
	.header {
		display: flex;
		align-items: center;

		justify-content: space-between;

		margin-top: 8px;
		margin-bottom: 16px;
	}
	.title {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 64px 0;
		border: 1px solid #ddd;
		border-radius: 12px;
		background-color: #fff;
		text-align: center;
	}

	.empty-state h3 {
		margin: 16px 0 8px;
		font-weight: 600;
		font-size: 18px;
	}

	.empty-state p {
		margin: 0 0 24px;
		color: var(--color-text-secondary);
	}
</style>

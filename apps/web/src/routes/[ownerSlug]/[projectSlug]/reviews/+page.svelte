<script lang="ts">
	import BranchIndexCard from '$lib/components/branches/BranchIndexCard.svelte';
	import { BranchService } from '@gitbutler/shared/branches/branchService';
	import { getBranchReviewsForRepository } from '@gitbutler/shared/branches/branchesPreview.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import type { ProjectParameters } from '@gitbutler/shared/routing/webRoutes.svelte';

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
			<div class="text">Branches shared for review</div>
			<div><Badge>{brancheses.length || 0}</Badge></div>
		</div>

		<table class="commits-table">
			<thead>
				<tr>
					<th><div>Status</div></th>
					<th><div>Name</div></th>
					<th><div>UUID</div></th>
					<th><div>Branch commits</div></th>
					<th><div>Status</div></th>
					<th><div>Last update</div></th>
					<th><div>Authors</div></th>
					<th><div>Version</div></th>
				</tr>
			</thead>
			<tbody class="pretty">
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
			</tbody>
		</table>
	{/snippet}
</Loading>

<style>
	.title {
		display: flex;
		align-items: center;
		margin-bottom: 1.5rem;
		gap: 10px;
	}
	.title > .text {
		font-weight: bold;
	}
</style>

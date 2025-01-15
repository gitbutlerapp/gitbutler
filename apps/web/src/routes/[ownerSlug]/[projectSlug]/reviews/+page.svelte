<script lang="ts">
	import BranchIndexCard from '$lib/components/branches/BranchIndexCard.svelte';
	import { BranchService } from '@gitbutler/shared/branches/branchService';
	import { getBranchReviews } from '@gitbutler/shared/branches/branchesPreview.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { isFound } from '@gitbutler/shared/network/loadable';
	import { lookupProject } from '@gitbutler/shared/organizations/repositoryIdLookupPreview.svelte';
	import { RepositoryIdLookupService } from '@gitbutler/shared/organizations/repositoryIdLookupService';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import type { ProjectParameters } from '$lib/project/types';

	interface Props {
		data: ProjectParameters;
	}

	let { data }: Props = $props();

	const repositoryIdLookupService = getContext(RepositoryIdLookupService);
	const branchService = getContext(BranchService);
	const appState = getContext(AppState);

	const repositoryId = $derived(
		lookupProject(appState, repositoryIdLookupService, data.ownerSlug, data.projectSlug)
	);
	const branches = $derived(
		isFound(repositoryId.current)
			? getBranchReviews(appState, branchService, repositoryId.current.value)
			: undefined
	);
</script>

<h2>{data.ownerSlug}/{data.projectSlug}</h2>

<Loading loadable={repositoryId.current}>
	{#snippet children(repositoryId)}
		<h3>Branches shared for review <Badge>{branches?.current?.length || 0}</Badge></h3>

		<table class="branches-table">
			<thead>
				<tr>
					<th>Seq.</th>
					<th>Name</th>
					<th>UUID</th>
					<th>Branch commits</th>
					<th>Status</th>
					<th>Last update</th>
					<th>Authors</th>
					<th>Version</th>
				</tr>
			</thead>
			<tbody>
				{#each branches?.current || [] as branch, index}
					<BranchIndexCard
						{repositoryId}
						linkParams={data}
						branchId={branch.id}
						roundedTop={index !== 0}
						roundedBottom={true}
					/>
				{/each}
			</tbody>
		</table>
	{/snippet}
</Loading>

<style lang="postcss">
	.branches-table {
		th {
			text-align: left;
			padding: 16px;

			border-top: 1px solid var(--clr-border-2);
			border-bottom: 1px solid var(--clr-border-2);
			overflow: hidden;

			&:first-child {
				border-left: 1px solid var(--clr-border-2);
				border-top-left-radius: var(--radius-m);
			}

			&:last-child {
				border-right: 1px solid var(--clr-border-2);
				border-top-right-radius: var(--radius-m);
			}
		}
	}
</style>

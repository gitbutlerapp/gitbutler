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

<h2>Reviews page: {data.ownerSlug}/{data.projectSlug}</h2>

<Loading loadable={repositoryId.current}>
	{#snippet children(repositoryId)}
		<p>{repositoryId}</p>

		{#each branches?.current || [] as branch}
			<BranchIndexCard {repositoryId} linkParams={data} branchId={branch.id} />
		{/each}
	{/snippet}
</Loading>

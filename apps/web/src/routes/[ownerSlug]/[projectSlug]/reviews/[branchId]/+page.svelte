<script lang="ts">
	import ChangeIndexCard from '$lib/components/changes/ChangeIndexCard.svelte';
	import { BranchService } from '@gitbutler/shared/branches/branchService';
	import { getBranchReview } from '@gitbutler/shared/branches/branchesPreview.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { isFound, and } from '@gitbutler/shared/network/loadable';
	import { lookupProject } from '@gitbutler/shared/organizations/repositoryIdLookupPreview.svelte';
	import { RepositoryIdLookupService } from '@gitbutler/shared/organizations/repositoryIdLookupService';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import type { ProjectReviewParameters } from '$lib/project/types';

	interface Props {
		data: ProjectReviewParameters;
	}

	let { data }: Props = $props();

	const repositoryIdLookupService = getContext(RepositoryIdLookupService);
	const branchService = getContext(BranchService);
	const appState = getContext(AppState);

	const repositoryId = $derived(
		lookupProject(appState, repositoryIdLookupService, data.ownerSlug, data.projectSlug)
	);
	const branch = $derived(
		isFound(repositoryId.current)
			? getBranchReview(appState, branchService, repositoryId.current.value, data.branchId)
			: undefined
	);
</script>

<h2>Review page: {data.ownerSlug}/{data.projectSlug} {data.branchId}</h2>

<Loading loadable={and(repositoryId.current, branch?.current)}>
	{#snippet children(branch)}
		<div>
			<h1>{branch.title}</h1>
			<p>Created at: {branch.createdAt}</p>

			{#each branch.patchIds || [] as changeId}
				<ChangeIndexCard {changeId} params={data} />
			{/each}
		</div>
	{/snippet}
</Loading>

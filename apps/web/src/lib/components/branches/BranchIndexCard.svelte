<script lang="ts">
	import { BranchService } from '@gitbutler/shared/branches/branchService';
	import { getBranchReview } from '@gitbutler/shared/branches/branchesPreview.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import type { ProjectParameters } from '$lib/project/types';

	type Props = {
		repositoryId: string;
		branchId: string;
		linkParams: ProjectParameters;
	};

	const { branchId, repositoryId, linkParams }: Props = $props();

	const appState = getContext(AppState);
	const branchService = getContext(BranchService);

	const branch = $derived(getBranchReview(appState, branchService, repositoryId, branchId));
</script>

<Loading loadable={branch.current}>
	{#snippet children(branch)}
		<a href={`/${linkParams.ownerSlug}/${linkParams.projectSlug}/reviews/${branchId}`}>
			<div class="card">
				<p>title: {branch.title}</p>
				<p>status: {branch.status}</p>
				<p>size: {branch.stackSize}</p>
			</div>
		</a>
	{/snippet}
</Loading>

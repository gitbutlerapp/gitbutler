<script lang="ts">
	import { ProjectService } from '$lib/project/projects';
	import { BranchService } from '@gitbutler/shared/branches/branchService';
	import { getBranchReviews } from '@gitbutler/shared/branches/branchesPreview.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { readableToReactive } from '@gitbutler/shared/reactiveUtils.svelte';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';

	const appState = getContext(AppState);
	const branchService = getContext(BranchService);
	const projectService = getContext(ProjectService);

	const project = readableToReactive(projectService.project);
	const repositoryId = $derived(project.current?.api?.repository_id);

	const branchReviews = $derived(
		repositoryId ? getBranchReviews(appState, branchService, repositoryId) : undefined
	);
</script>

<div class="series-container">
	<h2 class="text-head-24 heading">Your branches:</h2>

	{#if branchReviews?.current}
		{#each branchReviews.current as review}
			<Loading loadable={review}>
				{#snippet children(review)}
					<div>
						<p>{review.title}</p>
					</div>
				{/snippet}
			</Loading>
		{/each}
	{/if}
</div>

<style lang="postcss">
	.heading {
		margin-bottom: 16px;
	}

	.series-container {
		display: flex;
		flex-direction: column;

		max-width: 600px;
		width: 100%;

		margin: 24px auto;
	}
</style>

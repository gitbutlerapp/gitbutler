<script lang="ts">
	import { ProjectService } from '$lib/backend/projects';
	import BranchReviewCard from '@gitbutler/shared/branchReviews/BranchReviewCard.svelte';
	import { getBranchReviews } from '@gitbutler/shared/branchReviews/branchReviewPresenter.svelte';
	import { BranchReviewService } from '@gitbutler/shared/branchReviews/branchReviewService';
	import { getContext } from '@gitbutler/shared/context';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';

	const branchReivewService = getContext(BranchReviewService);
	const appState = getContext(AppState);
	const projectService = getContext(ProjectService);
	const project = projectService.project;

	const branchReviews = $derived.by(() => {
		if (!$project?.api) return undefined;

		return getBranchReviews(appState, branchReivewService, $project.api.repository_id);
	});
</script>

{#if $project?.api}
	<div>
		<div>
			{#if branchReviews}
				{#each branchReviews.current as branchReview}
					<BranchReviewCard
						repositoryId={$project.api.repository_id}
						branchId={branchReview.branchId}
					/>
				{/each}
			{:else}
				<p>Loading...</p>
			{/if}
		</div>
	</div>
{:else}
	<p>Project not found</p>
{/if}

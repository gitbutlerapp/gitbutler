<script lang="ts">
	import { getBranchReview } from '$lib/branchReviews/branchReviewPresenter.svelte';

	type Props = {
		branchId: string;
		repositoryId: string;
	};

	const { branchId, repositoryId }: Props = $props();

	const branchReview = $derived.by(() => {
		if (!branchId || !repositoryId) return undefined;

		return getBranchReview(branchId, repositoryId);
	});
</script>

<div>{branchReview?.current}</div>

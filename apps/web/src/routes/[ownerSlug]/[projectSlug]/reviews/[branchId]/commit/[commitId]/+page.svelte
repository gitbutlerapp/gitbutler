<script lang="ts">
	import ChatComponent from '$lib/components/ChatComponent.svelte';
	import type { ProjectReviewCommitParameters } from '$lib/project/types';

	interface Props {
		data: ProjectReviewCommitParameters;
	}

	let { data }: Props = $props();
</script>

<h2>Review page: {data.ownerSlug}/{data.projectSlug} {data.branchId}/{data.commitId}</h2>

<div class="review-page">
	<div class="review-main-content">the main area with the diffs</div>
	<div class="review-chat">
		<ChatComponent />
	</div>
</div>

<style>
	.review-page {
		display: flex;
		width: 100%;
		flex-grow: 1;
	}

	.review-main-content {
		width: 100%;
	}

	.review-chat {
		width: 100%;
	}
</style>

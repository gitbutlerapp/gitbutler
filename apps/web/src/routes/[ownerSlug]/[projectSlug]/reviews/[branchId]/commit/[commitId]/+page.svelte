<script lang="ts">
	import ChatComponent from '$lib/components/ChatComponent.svelte';
	import type { ProjectReviewCommitParameters } from '$lib/project/types';

	interface Props {
		data: ProjectReviewCommitParameters;
	}

	let { data }: Props = $props();
</script>

<h2>Review page: {data.ownerSlug}/{data.projectSlug} {data.branchId}/{data.commitId}</h2>

<div>
	<div>the main area with the diffs</div>
	<div>
		<ChatComponent />
	</div>
</div>

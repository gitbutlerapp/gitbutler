<script lang="ts">
	import type { ProjectReviewCommitParameters } from '../../../../../types';

	interface Props {
		data: ProjectReviewCommitParameters;
	}

	let { data }: Props = $props();
</script>

<h2>Review page: {data.ownerSlug}/{data.projectSlug} {data.branchId}/{data.commitId}</h2>

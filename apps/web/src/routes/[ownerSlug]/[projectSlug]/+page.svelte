<script lang="ts">
	import { projectReviewPath, type ProjectParameters } from '$lib/routing';

	interface Props {
		data: ProjectParameters;
	}

	let { data }: Props = $props();
</script>

<h2>Project page: {data.ownerSlug}/{data.projectSlug}</h2>

<a href={projectReviewPath(data)}>Project Reviews</a>

<script lang="ts">
	import type { ProjectParameters } from '$lib/project/types';

	interface Props {
		data: ProjectParameters;
	}

	let { data }: Props = $props();
</script>

<h2>Project page: {data.ownerSlug}/{data.projectSlug}</h2>

<a href={`/${data.ownerSlug}/${data.projectSlug}/reviews`}>Project Reviews</a>

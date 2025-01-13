<script lang="ts">
	import type { ProjectParameters } from '$lib/project/types';

	interface Props {
		data: ProjectParameters;
	}

	let { data }: Props = $props();
</script>

<h2>Reviews page: {data.ownerSlug}/{data.projectSlug}</h2>

<script lang="ts">
	import type { ProjectParameters } from '../../types';

	interface Props {
		data: ProjectParameters;
	}

	let { data }: Props = $props();
</script>

<h2>Reviews page: {data.ownerSlug}/{data.projectSlug}</h2>

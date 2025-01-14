<script lang="ts">
	import type { OwnerParameters } from '$lib/project/types';

	interface Props {
		data: OwnerParameters;
	}

	let { data }: Props = $props();
</script>

<h2>Owner: {data.ownerSlug}</h2>

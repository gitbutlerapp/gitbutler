<script lang="ts">
	import type { OwnerParameters } from '$lib/routing';

	interface Props {
		data: OwnerParameters;
	}

	let { data }: Props = $props();
</script>

<h2>Owner: {data.ownerSlug}</h2>

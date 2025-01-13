<script lang="ts">
	import type { OwnerParameters } from './types';

	interface Props {
		data: OwnerParameters;
	}

	let { data }: Props = $props();
</script>

<h2>Owner: {data.ownerSlug}</h2>

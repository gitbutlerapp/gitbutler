<script lang="ts">
	import type { OwnerParameters } from '@gitbutler/shared/routing/webRoutes';

	interface Props {
		data: OwnerParameters;
	}

	let { data }: Props = $props();
</script>

<h2>Owner: {data.ownerSlug}</h2>

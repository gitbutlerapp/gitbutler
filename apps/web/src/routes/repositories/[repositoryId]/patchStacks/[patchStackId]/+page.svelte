<script lang="ts">
	import CloudPatchStackShow from '@gitbutler/shared/cloud/stacks/CloudPatchStackShow.svelte';
	import { page } from '$app/stores';

	const patchStackId = $derived($page.params.patchStackId);
</script>

{#if patchStackId}
	<CloudPatchStackShow {patchStackId} />
{/if}

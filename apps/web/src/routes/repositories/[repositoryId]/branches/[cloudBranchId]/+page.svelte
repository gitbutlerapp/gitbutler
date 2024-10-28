<script lang="ts">
	import CloudBranchShow from '@gitbutler/shared/cloud/stacks/CloudBranchShow.svelte';
	import { page } from '$app/stores';

	const cloudBranchId = $derived($page.params.cloudBranchId);
</script>

{#if cloudBranchId}
	<CloudBranchShow {cloudBranchId} />
{/if}

<script lang="ts">
	import CloudBranchShow from '@gitbutler/shared/cloud/stacks/CloudBranchShow.svelte';
	import { page } from '$app/stores';

	const cloudBranchId = $derived($page.params.cloudBranchId);
</script>

<div class="patch-stack-container">
	{#if cloudBranchId}
		<CloudBranchShow {cloudBranchId} />
	{/if}
</div>

<style lang="postcss">
	.patch-stack-container {
		display: flex;
		flex-direction: column;

		max-width: 600px;
		width: 100%;

		margin: 24px auto;
	}
</style>

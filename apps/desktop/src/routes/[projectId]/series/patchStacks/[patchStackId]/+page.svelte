<script lang="ts">
	import CloudPatchStackShow from '@gitbutler/shared/cloud/stacks/CloudPatchStackShow.svelte';
	import { page } from '$app/stores';

	const patchStackId = $derived($page.params.patchStackId);
</script>

<div class="patch-stack-container">
	{#if patchStackId}
		<CloudPatchStackShow {patchStackId} />
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

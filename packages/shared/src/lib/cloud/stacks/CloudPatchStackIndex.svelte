<script lang="ts">
	import { CloudPatchStacksService } from '$lib/cloud/stacks/service';
	import { getContext } from '$lib/context';

	/**
	 * Expects the following contexts:
	 * - PatchStacksService
	 */

	const patchStacksService = getContext(CloudPatchStacksService);
	const patchStacks = $derived(patchStacksService.patchStacks);
</script>

{#if $patchStacks}
	{#each $patchStacks as patchStack}
		<div>{patchStack.title}</div>
	{/each}
{/if}

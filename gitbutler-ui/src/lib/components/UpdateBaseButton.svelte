<script lang="ts">
	import Tag from './Tag.svelte';
	import { getContext } from '$lib/utils/context';
	import * as toasts from '$lib/utils/toasts';
	import { BranchController } from '$lib/vbranches/branchController';

	const branchController = getContext(BranchController);

	let loading = false;
</script>

<Tag
	style="error"
	kind="solid"
	help="Merge upstream commits into common base"
	clickable
	on:click={async () => {
		loading = true;
		try {
			await branchController.updateBaseBranch();
		} catch {
			toasts.error('Failed update workspace');
		} finally {
			loading = false;
		}
	}}
>
	{#if loading}
		busy...
	{:else}
		Update
	{/if}
</Tag>

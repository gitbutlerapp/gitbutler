<script lang="ts">
	import Tag from './Tag.svelte';
	import { showError } from '$lib/notifications/toasts';
	import { getContext } from '$lib/utils/context';
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
		} catch (err) {
			showError('Failed update workspace', err);
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

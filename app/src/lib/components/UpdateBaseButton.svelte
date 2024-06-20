<script lang="ts">
	import { showInfo, showError } from '$lib/notifications/toasts';
	import Button from '$lib/shared/Button.svelte';
	import { getContext } from '$lib/utils/context';
	import { BranchController } from '$lib/vbranches/branchController';

	const branchController = getContext(BranchController);

	let loading = false;
</script>

<Button
	size="tag"
	style="error"
	kind="solid"
	help="Merge upstream commits into common base"
	on:click={async () => {
		loading = true;
		try {
			let infoText = await branchController.updateBaseBranch();
			if (infoText) {
				showInfo('Stashed conflicting branches', infoText);
			}
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
</Button>

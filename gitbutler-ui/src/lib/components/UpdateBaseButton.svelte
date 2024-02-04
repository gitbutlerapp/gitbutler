<script lang="ts">
	import Tag from './Tag.svelte';
	import * as toasts from '$lib/utils/toasts';
	import type { BranchController } from '$lib/vbranches/branchController';

	export let branchController: BranchController;

	let loading = false;
</script>

<Tag
	color="error"
	help="Merge upstream commits into common base"
	filled
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

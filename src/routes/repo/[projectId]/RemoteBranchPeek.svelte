<script lang="ts">
	import Button from '$lib/components/Button/Button.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { BranchData } from '$lib/vbranches/types';

	export let branch: BranchData | undefined;
	export let branchController: BranchController;
</script>

{#if branch != undefined}
	<div class="p-4 text-center">
		<h1 class="mb-2 text-xl font-medium">
			{branch.name.replace('refs/remotes/', '').replace('origin/', '').replace('refs/heads/', '')}
		</h1>
		<Button
			color="purple"
			height="small"
			on:click={() => branch && branchController.createvBranchFromBranch(branch.name)}
		>
			Apply
		</Button>
	</div>
{/if}

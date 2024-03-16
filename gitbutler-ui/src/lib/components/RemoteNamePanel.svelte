<script lang="ts">
	import { getContextByClass } from '$lib/utils/context';
	import { BranchController } from '$lib/vbranches/branchController';
	import type { RemoteBranch } from '$lib/vbranches/types';

	export let branch: RemoteBranch;
	export let branchId: string;

	const branchController = getContextByClass(BranchController);

	let remoteName = '';
	let remoteBranchName = '';

	$: if (branch) {
		let parts = branch.name.replace('refs/remotes/', '').split('/');
		remoteName = parts[0];
		// remoteBranchName is the rest
		let rbn = parts.slice(1).join('/');
		if (rbn != remoteBranchName) {
			remoteBranchName = rbn;
		}
	} else {
		remoteName = '';
		remoteBranchName = '';
	}

	function handleUpdateName(e: any) {
		let newBranchName = e.target?.value;
		branchController.updateBranchRemoteName(branchId, newBranchName);
	}
</script>

<div class="text-color-1 flex flex-row p-2 font-mono">
	{#if remoteName}
		<div class="p-2">{remoteName}/</div>
	{/if}
	<input
		autocomplete="off"
		autocorrect="off"
		spellcheck="true"
		value={remoteBranchName}
		on:change={handleUpdateName}
		name="remoteName"
		class="text-color-2 ring-gray-300 focus:ring-indigo-600 block w-full rounded-md border-0 px-2 py-1.5 shadow-sm ring-1 ring-inset placeholder:text-gray-400 focus:ring-2 focus:ring-inset sm:text-sm sm:leading-6"
		placeholder="Remote branch name (optional)"
	/>
</div>

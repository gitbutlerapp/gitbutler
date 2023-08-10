<script lang="ts">
	import Button from '$lib/components/Button/Button.svelte';
	import Modal from '$lib/components/Modal';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { Branch } from '$lib/vbranches/types';

	export let branch: Branch | undefined;
	export let branchController: BranchController;

	let applyConflictedModal: Modal;
	let deleteBranchModal: Modal;

	function toggleBranch(branch: Branch | undefined) {
		if (!branch) {
			return;
		} else if (!branch.baseCurrent) {
			applyConflictedModal.show(branch);
		} else if (branch.active) {
			branchController.unapplyBranch(branch.id);
		} else {
			branchController.applyBranch(branch.id);
		}
	}
</script>

<!-- Apply conflicted branch modal -->

{#if branch != undefined}
	<div class="p-4 text-center">
		<h1 class="mb-2 text-xl font-medium">{branch.name}</h1>
		<h2 class="mb-4 text-lg text-light-800">
			status: {branch.active ? 'applied' : 'unapplied'}
		</h2>
		<Button color="purple" height="small" on:click={() => toggleBranch(branch)}>
			{branch.active ? 'Unapply' : 'Apply'}
		</Button>
		{#if !branch.active}
			<Button color="purple" height="small" on:click={() => deleteBranchModal.show(branch)}>
				Delete
			</Button>
		{/if}
	</div>

	<Modal width="small" bind:this={applyConflictedModal}>
		<svelte:fragment slot="title">Merge conflicts</svelte:fragment>
		<p>Applying this branch will introduce merge conflicts.</p>
		<svelte:fragment slot="controls" let:item let:close>
			<Button height="small" kind="outlined" on:click={close}>Cancel</Button>
			<Button
				height="small"
				color="purple"
				on:click={() => {
					branchController.applyBranch(item.id);
					close();
				}}
			>
				Update
			</Button>
		</svelte:fragment>
	</Modal>

	<!-- Delete branch confirmation modal -->

	<Modal width="small" bind:this={deleteBranchModal} let:item>
		<svelte:fragment slot="title">Delete branch</svelte:fragment>
		<div>
			Deleting <code>{item.name}</code> cannot be undone.
		</div>
		<svelte:fragment slot="controls" let:close let:item>
			<Button height="small" kind="outlined" on:click={close}>Cancel</Button>
			<Button
				height="small"
				color="destructive"
				on:click={() => {
					branchController.deleteBranch(item.id);
					close();
				}}
			>
				Delete
			</Button>
		</svelte:fragment>
	</Modal>
{/if}

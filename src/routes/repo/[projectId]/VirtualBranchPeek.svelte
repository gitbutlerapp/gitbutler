<script lang="ts">
	import Button from '$lib/components/Button/Button.svelte';
	import Modal from '$lib/components/Modal';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranch, Branch } from '$lib/vbranches/types';
	import CommitCard from './CommitCard.svelte';

	export let branch: Branch | undefined;
	export let branchController: BranchController;
	export let base: BaseBranch | undefined;

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
	<div class="flex w-full max-w-full flex-col items-center gap-y-4 overflow-hidden p-4">
		<h1 class="text-xl font-medium">{branch.name}</h1>
		<h2 class="text-lg text-light-800 dark:text-dark-200">
			status: {branch.active ? 'applied' : 'unapplied'}
		</h2>
		<div class="flex gap-x-4">
			<Button color="purple" height="small" on:click={() => toggleBranch(branch)}>
				{branch.active ? 'Unapply' : 'Apply'}
			</Button>
			{#if !branch.active}
				<Button color="purple" height="small" on:click={() => deleteBranchModal.show(branch)}>
					Delete
				</Button>
			{/if}
		</div>
		{#if branch.commits && branch.commits.length > 0}
			<div class="flex w-full flex-col gap-y-2">
				{#each branch.commits as commit}
					<CommitCard {commit} url={base?.commitUrl(commit.id)} />
				{/each}
			</div>
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

<script lang="ts">
	import Button from '$lib/components/Button/Button.svelte';
	import Modal from '$lib/components/Modal';
	import Tooltip from '$lib/components/Tooltip/Tooltip.svelte';
	import IconDelete from '$lib/icons/IconDelete.svelte';
	import type { BranchController } from '$lib/vbranches/branchController';
	import type { BaseBranch, Branch } from '$lib/vbranches/types';
	import CommitCard from './CommitCard.svelte';

	export let branch: Branch | undefined;
	export let branchController: BranchController;
	export let base: BaseBranch | undefined;

	let applyConflictedModal: Modal;
	let deleteBranchModal: Modal;
	$: notesRows = branch ? Math.min(12, Math.max(2, branch.notes.split('\n').length)) : 2;

	function handleUpdateNotes() {
		if (branch) {
			notesRows = Math.min(12, Math.max(2, branch.notes.split('\n').length));
			branchController.updateBranchNotes(branch.id, branch.notes);
		}
	}

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

{#if branch != undefined}
	<div class="flex w-full max-w-full flex-col gap-y-4 p-4">
		<div class="flex">
			<div class="flex-grow items-center">
				<p class="text-lg font-bold" title="name of virtual branch">{branch.name}</p>
				<p class="text-light-700 dark:text-dark-200" title="upstream target">
					{branch.upstream?.replace('refs/remotes/', '') || ''}
				</p>
			</div>
			<div>
				<button
					class="p-0 align-middle text-light-500 hover:text-light-700 disabled:cursor-not-allowed disabled:text-light-200 dark:text-dark-400 hover:dark:text-dark-200 dark:disabled:text-dark-400"
					disabled={!branch.active}
					title={branch.active ? 'branch cannot be deleted while applied' : 'deletes this branch'}
					on:click={() => deleteBranchModal.show(branch)}
				>
					<IconDelete class="h-4 w-4" />
				</button>
			</div>
		</div>
		<div>
			{#if branch.active}
				<div class="inline-block rounded-lg bg-green-500 px-2 py-0.5 font-bold dark:bg-green-700">
					<span class="text-white">applied</span>
				</div>
			{:else if !branch.mergeable}
				<!-- use of relative is for tooltip rendering -->
				<div
					class="relative inline-block rounded-lg bg-red-500 px-2 py-0.5 font-bold dark:bg-red-700"
				>
					<Tooltip label="Canflicts with changes in your working directory, cannot be applied">
						<span class="text-white">cannot be applied</span>
					</Tooltip>
				</div>
			{:else if !branch.baseCurrent}
				<div class="inline-block rounded-lg bg-yellow-500 px-2 py-0.5 font-bold dark:bg-yellow-600">
					<Tooltip label="Will introduce merge conflicts if applied">
						<span class="">will cause merge conflicts</span>
					</Tooltip>
				</div>
			{:else}
				<div class="inline-block rounded-lg bg-light-600 px-2 py-0.5 font-bold dark:bg-dark-300">
					<span class="text-white">not applied</span>
				</div>
			{/if}
		</div>
		<div class="w-full">
			<textarea
				autocomplete="off"
				autocorrect="off"
				spellcheck="true"
				bind:value={branch.notes}
				on:change={handleUpdateNotes}
				name="commit-description"
				class="quick-commit-input outline-none-important w-full resize-none rounded border border-zinc-100 bg-transparent p-2 text-zinc-800"
				placeholder="Branch notes (optional)"
				rows={notesRows}
			/>
		</div>
		{#if branch.commits && branch.commits.length > 0}
			<div class="flex w-full flex-col gap-y-2">
				{#each branch.commits as commit}
					<CommitCard {commit} url={base?.commitUrl(commit.id)} />
				{/each}
			</div>
		{/if}
	</div>

	<!-- Apply conflicted branch modal -->

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

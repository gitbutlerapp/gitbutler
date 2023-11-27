<script lang="ts">
	import type { PageData } from './$types';
	import { page } from '$app/stores';
	import BranchLane from '../../components/BranchLane.svelte';
	import Button from '$lib/components/Button.svelte';
	import IconButton from '$lib/components/IconButton.svelte';
	import type { Branch } from '$lib/vbranches/types';
	import Modal from '$lib/components/Modal.svelte';

	export let data: PageData;

	let applyConflictedModal: Modal;
	let deleteBranchModal: Modal;

	$: projectId = data.projectId;
	$: user$ = data.user$;
	$: githubContext$ = data.githubContext$;
	$: cloud = data.cloud;

	$: branchController = data.branchController;
	$: vbranchService = data.vbranchService;
	$: baseBranchService = data.baseBranchService;
	$: baseBranch$ = baseBranchService.base$;

	$: branches$ = vbranchService.branches$;
	$: error$ = vbranchService.branchesError$;
	$: branch = $branches$?.find((b) => b.id == $page.params.branchId);

	function applyBranch(branch: Branch) {
		if (!branch.baseCurrent) {
			applyConflictedModal.show(branch);
		} else {
			branchController.applyBranch(branch.id);
		}
	}
</script>

<div class="h-full flex-grow overflow-y-auto overscroll-none p-3">
	{#if $error$}
		<p>Error...</p>
	{:else if !$branches$}
		<p>Loading...</p>
	{:else if branch}
		{#await branch.isMergeable then isMergeable}
			{#if isMergeable}
				<Button
					class="w-20"
					height="small"
					kind="outlined"
					color="purple"
					on:click={() => branch && applyBranch(branch)}
				>
					<span class="purple"> Apply </span>
				</Button>
			{/if}
		{/await}
		<IconButton
			icon="question-mark"
			title="delete branch"
			on:click={() => deleteBranchModal.show(branch)}
		/>
		<BranchLane
			{branch}
			{branchController}
			base={$baseBranch$}
			{cloud}
			{projectId}
			maximized={true}
			cloudEnabled={false}
			readonly={true}
			githubContext={$githubContext$}
			user={$user$}
		/>
	{:else}
		<p>Branch no longer exists</p>
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

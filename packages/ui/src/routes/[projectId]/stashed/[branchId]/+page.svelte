<script lang="ts">
	import type { PageData } from './$types';
	import { page } from '$app/stores';
	import BranchLane from '../../components/BranchLane.svelte';
	import Button from '$lib/components/Button.svelte';
	import IconButton from '$lib/components/IconButton.svelte';
	import type { Branch } from '$lib/vbranches/types';
	import Modal from '$lib/components/Modal.svelte';
	import Tooltip from '$lib/components/Tooltip.svelte';

	export let data: PageData;

	let applyConflictedModal: Modal;
	let deleteBranchModal: Modal;

	$: projectId = data.projectId;
	$: user$ = data.user$;
	$: githubContext$ = data.githubContext$;
	$: cloud = data.cloud;
	$: project$ = data.project$;

	$: branchController = data.branchController;
	$: vbranchService = data.vbranchService;
	$: baseBranchService = data.baseBranchService;
	$: baseBranch$ = baseBranchService.base$;

	$: branches$ = vbranchService.branches$;
	$: error$ = vbranchService.branchesError$;
	$: branch = $branches$?.find((b) => b.id == $page.params.branchId);

	function applyBranch(branch: Branch) {
		if (!branch.isMergeable) {
			applyConflictedModal.show(branch);
		} else {
			branchController.applyBranch(branch.id);
		}
	}
</script>

<div class="flex h-full flex-col p-3">
	{#if $error$}
		<p>Error...</p>
	{:else if !$branches$}
		<p>Loading...</p>
	{:else if branch}
		<div class="flex items-center">
			<Button kind="outlined" color="primary" on:click={() => branch && applyBranch(branch)}>
				<span class="purple"> Apply </span>
			</Button>
			<IconButton
				icon="question-mark"
				title="delete branch"
				on:click={() => deleteBranchModal.show(branch)}
			/>
			{#await branch.isMergeable then isMergeable}
				{#if isMergeable}
					<Tooltip
						timeoutMilliseconds={100}
						label="Applying this branch will add merge conflict markers that you will have to resolve"
					>
						<div
							class="flex cursor-default select-none rounded bg-yellow-300 px-2 py-0.5 dark:bg-yellow-800"
						>
							Conflicts with Applied Branches
						</div>
					</Tooltip>
				{/if}
			{/await}
		</div>
		<div class="h-full">
			<BranchLane
				{branch}
				{branchController}
				base={$baseBranch$}
				{cloud}
				{projectId}
				maximized={false}
				cloudEnabled={false}
				readonly={true}
				githubContext={$githubContext$}
				user={$user$}
				projectPath={$project$.path}
			/>
		</div>
	{:else}
		<p>Branch no longer exists</p>
	{/if}
</div>

<Modal width="small" bind:this={applyConflictedModal}>
	<svelte:fragment slot="title">Merge conflicts</svelte:fragment>
	<p>Applying this branch will introduce merge conflicts.</p>
	<svelte:fragment slot="controls" let:item let:close>
		<Button kind="outlined" on:click={close}>Cancel</Button>
		<Button
			color="primary"
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
		<Button kind="outlined" on:click={close}>Cancel</Button>
		<Button
			color="error"
			on:click={() => {
				branchController.deleteBranch(item.id);
				close();
			}}
		>
			Delete
		</Button>
	</svelte:fragment>
</Modal>

<script lang="ts">
	import type { PageData } from './$types';
	import { page } from '$app/stores';
	import BranchLane from '../../components/BranchLane.svelte';
	import Button from '$lib/components/Button.svelte';
	import IconButton from '$lib/components/IconButton.svelte';
	import type { Branch } from '$lib/vbranches/types';
	import Modal from '$lib/components/Modal.svelte';
	import Tooltip from '$lib/components/Tooltip.svelte';
	import { goto } from '$app/navigation';

	export let data: PageData;

	let applyConflictedModal: Modal;
	let deleteBranchModal: Modal;

	$: projectId = data.projectId;
	$: user$ = data.user$;
	$: cloud = data.cloud;
	$: project$ = data.project$;

	$: branchController = data.branchController;
	$: vbranchService = data.vbranchService;
	$: baseBranchService = data.baseBranchService;
	$: baseBranch$ = baseBranchService.base$;

	$: branches$ = vbranchService.branches$;
	$: error$ = vbranchService.branchesError$;
	$: branch = $branches$?.find((b) => b.id == $page.params.branchId);
	$: githubService = data.githubService;

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
		<div class="flex items-center justify-between pb-2">
			<Button
				kind="outlined"
				color="primary"
				on:click={() => {
					branch && applyBranch(branch);
					goto(`/${projectId}/board`);
				}}
			>
				<span class="purple"> Apply </span>
			</Button>
			<div class="flex items-center">
				{#await branch.isMergeable then isMergeable}
					{#if !isMergeable}
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
				<IconButton
					icon="cross"
					title="delete branch"
					on:click={() => deleteBranchModal.show(branch)}
				/>
			</div>
		</div>
		<div class="h-full">
			<BranchLane
				{branch}
				{branchController}
				base={$baseBranch$}
				{cloud}
				project={$project$}
				maximized={false}
				readonly={true}
				user={$user$}
				projectPath={$project$.path}
				{githubService}
			/>
		</div>
	{:else}
		<p>Branch no longer exists</p>
	{/if}
</div>

<Modal width="small" title="Merge conflicts" bind:this={applyConflictedModal}>
	<p>Applying this branch will introduce merge conflicts.</p>
	<svelte:fragment slot="controls" let:item let:close>
		<Button kind="outlined" color="neutral" on:click={close}>Cancel</Button>
		<Button
			color="primary"
			on:click={() => {
				branchController.applyBranch(item.id);
				close();
				goto(`/${projectId}/board`);
			}}
		>
			Update
		</Button>
	</svelte:fragment>
</Modal>

<Modal width="small" title="Delete branch" bind:this={deleteBranchModal} let:item>
	<div>
		Deleting <code>{item.name}</code> cannot be undone.
	</div>
	<svelte:fragment slot="controls" let:close let:item>
		<Button kind="outlined" color="neutral" on:click={close}>Cancel</Button>
		<Button
			color="error"
			on:click={() => {
				branchController.deleteBranch(item.id);
				close();
				goto(`/${projectId}/board`);
			}}
		>
			Delete
		</Button>
	</svelte:fragment>
</Modal>

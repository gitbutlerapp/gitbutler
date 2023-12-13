<script lang="ts">
	import Modal from '$lib/components/Modal.svelte';
	import Button from '$lib/components/Button.svelte';
	import { projectAiGenEnabled } from '$lib/config/config';
	import type { BranchController } from '$lib/vbranches/branchController';
	import { createEventDispatcher } from 'svelte';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import type { Branch } from '$lib/vbranches/types';

	export let branchController: BranchController;
	export let branch: Branch;
	export let projectId: string;
	export let visible: boolean;

	let deleteBranchModal: Modal;

	const dispatch = createEventDispatcher<{
		action: 'expand' | 'collapse' | 'generate-branch-name';
	}>();

	const aiGenEnabled = projectAiGenEnabled(projectId);
</script>

{#if visible}
	<ContextMenu>
		<ContextMenuSection>
			<ContextMenuItem
				label="Unapply"
				on:click={() => branch.id && branchController.unapplyBranch(branch.id)}
			/>

			<ContextMenuItem
				label="Delete"
				on:click={() => {
					deleteBranchModal.show(branch);
					visible = false;
				}}
			/>

			<ContextMenuItem
				label="Generate branch name"
				on:click={() => {
					dispatch('action', 'generate-branch-name');
					visible = false;
				}}
				disabled={!$aiGenEnabled || branch.files?.length == 0 || !branch.active}
			/>
		</ContextMenuSection>

		<ContextMenuSection>
			<ContextMenuItem
				label="Create branch before"
				on:click={() => {
					branchController.createBranch({ order: branch.order });
					visible = false;
				}}
			/>

			<ContextMenuItem
				label="Create branch after"
				on:click={() => {
					branchController.createBranch({ order: branch.order + 1 });
					visible = false;
				}}
			/>
		</ContextMenuSection>
	</ContextMenu>

	<Modal width="small" title="Delete branch" bind:this={deleteBranchModal} let:item={branch}>
		<div>
			Deleting <code>{branch.name}</code> cannot be undone.
		</div>
		<svelte:fragment slot="controls" let:close let:item={branch}>
			<Button kind="outlined" on:click={close}>Cancel</Button>
			<Button
				color="error"
				on:click={async () => {
					await branchController.deleteBranch(branch.id);
					visible = false;
				}}
			>
				Delete
			</Button>
		</svelte:fragment>
	</Modal>
{/if}

<style lang="postcss">
</style>

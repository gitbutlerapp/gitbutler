<script lang="ts">
	import { AIService } from '$lib/backend/aiService';
	import Button from '$lib/components/Button.svelte';
	import Modal from '$lib/components/Modal.svelte';
	import TextBox from '$lib/components/TextBox.svelte';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { UserService } from '$lib/stores/user';
	import { normalizeBranchName } from '$lib/utils/branch';
	import { getContextByClass } from '$lib/utils/context';
	import { BranchController } from '$lib/vbranches/branchController';
	import { createEventDispatcher } from 'svelte';
	import type { User } from '$lib/backend/cloud';
	import type { Branch } from '$lib/vbranches/types';

	export let branch: Branch;
	export let projectId: string;
	export let visible: boolean;
	export let isUnapplied = false;

	const branchController = getContextByClass(BranchController);
	const aiService = getContextByClass(AIService);
	const userService = getContextByClass(UserService);
	const user = userService.user;

	let deleteBranchModal: Modal;
	let renameRemoteModal: Modal;
	let newRemoteName: string;

	const dispatch = createEventDispatcher<{
		action: 'expand' | 'collapse' | 'generate-branch-name';
	}>();

	const aiGenEnabled = projectAiGenEnabled(projectId);

	$: commits = branch.commits;
	$: hasIntegratedCommits =
		commits.length > 0 ? commits.some((c) => c.status == 'integrated') : false;

	let aiConfigurationValid = false;

	$: setAIConfigurationValid($user);

	async function setAIConfigurationValid(user: User | undefined) {
		aiConfigurationValid = await aiService.validateConfiguration(user?.access_token);
	}
</script>

{#if visible}
	<ContextMenu>
		<ContextMenuSection>
			{#if !isUnapplied}
				<ContextMenuItem
					label="Unapply"
					on:click={() => {
						if (branch.id) branchController.unapplyBranch(branch.id);
						visible = false;
					}}
				/>
			{/if}

			<ContextMenuItem
				label="Delete"
				on:click={async () => {
					if (
						branch.name.toLowerCase().includes('virtual branch') &&
						commits.length == 0 &&
						branch.files?.length == 0
					) {
						await branchController.deleteBranch(branch.id);
					} else {
						deleteBranchModal.show(branch);
					}
					visible = false;
				}}
			/>

			<ContextMenuItem
				label="Generate branch name"
				on:click={() => {
					dispatch('action', 'generate-branch-name');
					visible = false;
				}}
				disabled={isUnapplied ||
					!($aiGenEnabled && aiConfigurationValid) ||
					branch.files?.length == 0 ||
					!branch.active}
			/>
		</ContextMenuSection>
		<ContextMenuSection>
			<ContextMenuItem
				label="Set remote branch name"
				disabled={isUnapplied || hasIntegratedCommits}
				on:click={() => {
					newRemoteName = branch.upstreamName || normalizeBranchName(branch.name) || '';
					visible = false;
					renameRemoteModal.show(branch);
				}}
			/>
		</ContextMenuSection>
		<ContextMenuSection>
			<ContextMenuItem
				label="Create branch to the left"
				on:click={() => {
					branchController.createBranch({ order: branch.order });
					visible = false;
				}}
			/>

			<ContextMenuItem
				label="Create branch to the right"
				on:click={() => {
					branchController.createBranch({ order: branch.order + 1 });
					visible = false;
				}}
			/>
		</ContextMenuSection>
	</ContextMenu>
{/if}

<Modal width="small" bind:this={renameRemoteModal}>
	<svelte:fragment let:close>
		<form
			id="branch-name-form"
			on:submit={() => {
				branchController.updateBranchRemoteName(branch.id, newRemoteName);
				close();
				visible = false;
			}}
		>
			<TextBox label="Remote branch name" id="newRemoteName" bind:value={newRemoteName} focus />
		</form>
	</svelte:fragment>

	<svelte:fragment slot="controls" let:close>
		<Button color="neutral" kind="outlined" on:click={close}>Cancel</Button>
		<Button color="primary" form="branch-name-form">Rename</Button>
	</svelte:fragment>
</Modal>

<Modal width="small" title="Delete branch" bind:this={deleteBranchModal} let:item={branch}>
	<div>
		Deleting <code>{branch.name}</code> cannot be undone.
	</div>
	<svelte:fragment slot="controls" let:close let:item={branch}>
		<Button kind="outlined" color="neutral" on:click={close}>Cancel</Button>
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

<style lang="postcss">
</style>

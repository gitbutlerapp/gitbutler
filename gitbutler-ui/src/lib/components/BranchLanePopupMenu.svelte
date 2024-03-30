<script lang="ts">
	import { AIService } from '$lib/backend/aiService';
	import { User } from '$lib/backend/cloud';
	import { Project } from '$lib/backend/projects';
	import Button from '$lib/components/Button.svelte';
	import Modal from '$lib/components/Modal.svelte';
	import TextBox from '$lib/components/TextBox.svelte';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { normalizeBranchName } from '$lib/utils/branch';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { BranchController } from '$lib/vbranches/branchController';
	import { Branch } from '$lib/vbranches/types';
	import { createEventDispatcher } from 'svelte';

	export let visible: boolean;
	export let isUnapplied = false;

	const user = getContextStore(User);
	const project = getContext(Project);
	const aiService = getContext(AIService);
	const branchStore = getContextStore(Branch);
	const aiGenEnabled = projectAiGenEnabled(project.id);
	const branchController = getContext(BranchController);

	let aiConfigurationValid = false;
	let deleteBranchModal: Modal;
	let renameRemoteModal: Modal;
	let newRemoteName: string;

	const dispatch = createEventDispatcher<{
		action: 'expand' | 'collapse' | 'generate-branch-name';
	}>();

	$: branch = $branchStore;
	$: commits = branch.commits;
	$: setAIConfigurationValid($user);
	$: hasIntegratedCommits = branch.integratedCommits.length > 0;

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

<Modal
	width="small"
	bind:this={renameRemoteModal}
	on:submit={() => {
		branchController.updateBranchRemoteName(branch.id, newRemoteName);
		renameRemoteModal.close();
		visible = false;
	}}
>
	<svelte:fragment>
		<TextBox label="Remote branch name" id="newRemoteName" bind:value={newRemoteName} focus />
	</svelte:fragment>

	<svelte:fragment slot="controls" let:close>
		<Button color="neutral" type="reset" kind="outlined" on:click={close}>Cancel</Button>
		<Button color="primary" type="submit">Rename</Button>
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

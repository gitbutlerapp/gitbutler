<script lang="ts">
	import { AIService } from '$lib/ai/service';
	import { Project } from '$lib/backend/projects';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { projectAiGenEnabled } from '$lib/config/config';
	import Button from '$lib/shared/Button.svelte';
	import Modal from '$lib/shared/Modal.svelte';
	import TextBox from '$lib/shared/TextBox.svelte';
	import Toggle from '$lib/shared/Toggle.svelte';
	import { User } from '$lib/stores/user';
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
	$: allowRebasing = branch.allowRebasing;

	async function toggleAllowRebasing() {
		branchController.updateBranchAllowRebasing(branch.id, !allowRebasing);
	}

	async function setAIConfigurationValid(user: User | undefined) {
		aiConfigurationValid = await aiService.validateConfiguration(user?.access_token);
	}

	function close() {
		visible = false;
	}
</script>

{#if visible}
	<ContextMenu>
		<ContextMenuSection>
			{#if !isUnapplied}
				<ContextMenuItem
					label="Collapse lane"
					on:click={() => {
						dispatch('action', 'collapse');
						close();
					}}
				/>
			{/if}
		</ContextMenuSection>
		<ContextMenuSection>
			{#if !isUnapplied}
				<ContextMenuItem
					label="Unapply"
					on:click={() => {
						if (branch.id) branchController.unapplyBranch(branch.id);
						close();
					}}
				/>
			{/if}

			<ContextMenuItem
				label="Delete"
				on:click={async () => {
					if (
						branch.name.toLowerCase().includes('virtual branch') &&
						commits.length === 0 &&
						branch.files?.length === 0
					) {
						await branchController.deleteBranch(branch.id);
					} else {
						deleteBranchModal.show(branch);
					}
					close();
				}}
			/>

			<ContextMenuItem
				label="Generate branch name"
				on:click={() => {
					dispatch('action', 'generate-branch-name');
					close();
				}}
				disabled={isUnapplied ||
					!($aiGenEnabled && aiConfigurationValid) ||
					branch.files?.length === 0 ||
					!branch.active}
			/>

			<ContextMenuItem label="Allow rebasing" on:click={toggleAllowRebasing}>
				<Toggle
					small
					slot="control"
					bind:checked={allowRebasing}
					on:click={toggleAllowRebasing}
					help="Having this enabled permits commit amending and reordering after a branch has been pushed, which would subsequently require force pushing"
				/>
			</ContextMenuItem>
		</ContextMenuSection>
		<ContextMenuSection>
			<ContextMenuItem
				label="Set remote branch name"
				disabled={isUnapplied}
				on:click={() => {
					newRemoteName = branch.upstreamName || normalizeBranchName(branch.name) || '';
					close();
					renameRemoteModal.show(branch);
				}}
			/>
		</ContextMenuSection>
		<ContextMenuSection>
			<ContextMenuItem
				label="Create branch to the left"
				on:click={() => {
					branchController.createBranch({ order: branch.order });
					close();
				}}
			/>

			<ContextMenuItem
				label="Create branch to the right"
				on:click={() => {
					branchController.createBranch({ order: branch.order + 1 });
					close();
				}}
			/>
		</ContextMenuSection>
	</ContextMenu>
{/if}

<Modal width="small" bind:this={renameRemoteModal}>
	<TextBox label="Remote branch name" id="newRemoteName" bind:value={newRemoteName} focus />

	{#snippet controls(close)}
		<Button style="ghost" outline type="reset" on:click={close}>Cancel</Button>
		<Button
			style="pop"
			kind="solid"
			on:click={() => {
				branchController.updateBranchRemoteName(branch.id, newRemoteName);
				close();
			}}
		>
			Rename
		</Button>
	{/snippet}
</Modal>

<Modal width="small" title="Delete branch" bind:this={deleteBranchModal}>
	{#snippet children(branch)}
		Are you sure you want to delete <code class="code-string">{branch.name}</code>?
	{/snippet}
	{#snippet controls(close)}
		<Button style="ghost" outline on:click={close}>Cancel</Button>
		<Button
			style="error"
			kind="solid"
			on:click={async () => {
				await branchController.deleteBranch(branch.id);
				close();
			}}
		>
			Delete
		</Button>
	{/snippet}
</Modal>

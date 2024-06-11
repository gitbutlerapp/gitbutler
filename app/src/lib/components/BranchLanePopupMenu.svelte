<script lang="ts">
	import { AIService } from '$lib/ai/service';
	import { Project } from '$lib/backend/projects';
	import Button from '$lib/components/Button.svelte';
	import Modal from '$lib/components/Modal.svelte';
	import Select from '$lib/components/Select.svelte';
	import SelectItem from '$lib/components/SelectItem.svelte';
	import TextBox from '$lib/components/TextBox.svelte';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { User } from '$lib/stores/user';
	import { normalizeBranchName } from '$lib/utils/branch';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { BranchController } from '$lib/vbranches/branchController';
	import { Branch, type NameConflictResolution } from '$lib/vbranches/types';
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

	async function setAIConfigurationValid(user: User | undefined) {
		aiConfigurationValid = await aiService.validateConfiguration(user?.access_token);
	}

	function close() {
		visible = false;
	}

	let unapplyBranchModal: Modal;

	type ResolutionVariants = NameConflictResolution['type'];

	const resolutions: { value: ResolutionVariants; label: string }[] = [
		{
			value: 'overwrite',
			label: 'Overwrite the existing branch'
		},
		{
			value: 'suffix',
			label: 'Suffix the branch name'
		},
		{
			value: 'rename',
			label: 'Use a new name'
		}
	];

	let selectedResolution: ResolutionVariants = resolutions[0].value;
	let newBranchName = '';

	function unapplyBranchWithSelectedResolution() {
		let resolution: NameConflictResolution | undefined;
		if (selectedResolution === 'rename') {
			resolution = {
				type: selectedResolution,
				value: newBranchName
			};
		} else {
			resolution = {
				type: selectedResolution,
				value: undefined
			};
		}

		branchController.convertToRealBranch(branch.id, resolution);

		unapplyBranchModal.close();
	}

	const remoteBranches = branchController.remoteBranchService.branches$;

	function tryUnapplyBranch() {
		if ($remoteBranches.find((b) => b.name.endsWith(normalizeBranchName(branch.name)))) {
			unapplyBranchModal.show();
		} else {
			// No resolution required
			branchController.convertToRealBranch(branch.id);
		}
	}
</script>

<Modal bind:this={unapplyBranchModal}>
	<div class="flow">
		<div class="modal-copy">
			<p class="text-base-15">There is already branch with the name</p>
			<Button size="tag" clickable={false}>{normalizeBranchName(branch.name)}</Button>
			<p class="text-base-15">.</p>
			<p class="text-base-15">Please choose how you want to resolve this:</p>
		</div>

		<Select
			items={resolutions}
			itemId={'value'}
			labelId={'label'}
			bind:selectedItemId={selectedResolution}
		>
			<SelectItem slot="template" let:item let:selected {selected}>
				{item.label}
			</SelectItem>
		</Select>
		{#if selectedResolution === 'rename'}
			<TextBox
				label="New branch name"
				id="newBranchName"
				bind:value={newBranchName}
				placeholder="Enter new branch name"
			/>
		{/if}
	</div>
	<svelte:fragment slot="controls">
		<Button style="ghost" outline on:click={() => unapplyBranchModal.close()}>Cancel</Button>
		<Button style="pop" kind="solid" grow on:click={unapplyBranchWithSelectedResolution}
			>Submit</Button
		>
	</svelte:fragment>
</Modal>

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
						tryUnapplyBranch();
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

<Modal
	width="small"
	bind:this={renameRemoteModal}
	on:submit={() => {
		branchController.updateBranchRemoteName(branch.id, newRemoteName);
		renameRemoteModal.close();
	}}
>
	<svelte:fragment>
		<TextBox label="Remote branch name" id="newRemoteName" bind:value={newRemoteName} focus />
	</svelte:fragment>

	<svelte:fragment slot="controls" let:close>
		<Button style="ghost" outline type="reset" on:click={close}>Cancel</Button>
		<Button style="pop" kind="solid" type="submit">Rename</Button>
	</svelte:fragment>
</Modal>

<Modal width="small" title="Delete branch" bind:this={deleteBranchModal} let:item={branch}>
	<svelte:fragment>
		Deleting <code class="code-string">{branch.name}</code> cannot be undone.
	</svelte:fragment>
	<svelte:fragment slot="controls" let:close let:item={branch}>
		<Button style="ghost" outline on:click={close}>Cancel</Button>
		<Button
			style="error"
			kind="solid"
			on:click={async () => {
				await branchController.deleteBranch(branch.id);
				deleteBranchModal.close();
			}}
		>
			Delete
		</Button>
	</svelte:fragment>
</Modal>

<style lang="postcss">
	.flow {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}
	.modal-copy {
		& > * {
			display: inline;
		}
	}
</style>

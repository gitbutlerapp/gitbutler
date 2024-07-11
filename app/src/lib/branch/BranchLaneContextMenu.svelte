<script lang="ts">
	import { AIService } from '$lib/ai/service';
	import { Project } from '$lib/backend/projects';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { projectAiGenEnabled } from '$lib/config/config';
	import Select from '$lib/select/Select.svelte';
	import SelectItem from '$lib/select/SelectItem.svelte';
	import Button from '$lib/shared/Button.svelte';
	import Modal from '$lib/shared/Modal.svelte';
	import TextBox from '$lib/shared/TextBox.svelte';
	import Toggle from '$lib/shared/Toggle.svelte';
	import { User } from '$lib/stores/user';
	import { normalizeBranchName } from '$lib/utils/branch';
	import { getContext, getContextStore } from '$lib/utils/context';
	import { BranchController } from '$lib/vbranches/branchController';
	import { Branch, type NameConflictResolution } from '$lib/vbranches/types';
	import { createEventDispatcher } from 'svelte';

	export let contextMenuEl: ContextMenu;
	export let target: HTMLElement;

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

	function setButtonCoppy() {
		switch (selectedResolution) {
			case 'overwrite':
				return 'Overwrite and unapply';
			case 'suffix':
				return 'Suffix and unapply';
			case 'rename':
				return 'Rename and unapply';
		}
	}
</script>

<Modal width="small" bind:this={unapplyBranchModal}>
	<div class="flow">
		<div class="modal-copy">
			<p class="text-base-14 text-semibold">
				"{normalizeBranchName(branch.name)}" branch already exists
			</p>

			<p class="text-base-body-13 modal-copy-caption">
				A branch with the same name already exists.
				<br />
				Please select a resolution:
			</p>
		</div>

		<Select
			value={selectedResolution}
			options={resolutions}
			onselect={(value) => {
				selectedResolution = value as ResolutionVariants;
			}}
		>
			{#snippet itemSnippet({ item, highlighted })}
				<SelectItem selected={item.value === selectedResolution} {highlighted}>
					{item.label}
				</SelectItem>
			{/snippet}
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
	{#snippet controls()}
		<Button style="ghost" outline on:click={() => unapplyBranchModal.close()}>Cancel</Button>
		<Button
			style="pop"
			kind="solid"
			on:click={unapplyBranchWithSelectedResolution}
			disabled={!newBranchName && selectedResolution === 'rename'}
		>
			{setButtonCoppy()}
		</Button>
	{/snippet}
</Modal>

<ContextMenu bind:this={contextMenuEl} {target}>
	<ContextMenuSection>
		<ContextMenuItem
			label="Collapse lane"
			on:click={() => {
				dispatch('action', 'collapse');
				contextMenuEl.close();
			}}
		/>
	</ContextMenuSection>
	<ContextMenuSection>
		<ContextMenuItem
			label="Unapply"
			on:click={() => {
				tryUnapplyBranch();
				contextMenuEl.close();
			}}
		/>

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
				contextMenuEl.close();
			}}
		/>

		<ContextMenuItem
			label="Generate branch name"
			on:click={() => {
				dispatch('action', 'generate-branch-name');
				contextMenuEl.close();
			}}
			disabled={!($aiGenEnabled && aiConfigurationValid) || branch.files?.length === 0}
		/>
	</ContextMenuSection>

	<ContextMenuSection>
		<ContextMenuItem
			label="Set remote branch name"
			on:click={() => {
				console.log('Set remote branch name');

				newRemoteName = branch.upstreamName || normalizeBranchName(branch.name) || '';
				renameRemoteModal.show(branch);
				contextMenuEl.close();
			}}
		/>
	</ContextMenuSection>

	<ContextMenuSection>
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
			label="Create branch to the left"
			on:click={() => {
				branchController.createBranch({ order: branch.order });
				contextMenuEl.close();
			}}
		/>

		<ContextMenuItem
			label="Create branch to the right"
			on:click={() => {
				branchController.createBranch({ order: branch.order + 1 });
				contextMenuEl.close();
			}}
		/>
	</ContextMenuSection>
</ContextMenu>

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

<style lang="postcss">
	.flow {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.modal-copy {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.modal-copy-caption {
		color: var(--clr-text-2);
	}
</style>

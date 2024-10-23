<script lang="ts">
	import { AIService } from '$lib/ai/service';
	import { Project } from '$lib/backend/projects';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { projectAiGenEnabled } from '$lib/config/config';
	import TextBox from '$lib/shared/TextBox.svelte';
	import { BranchController } from '$lib/vbranches/branchController';
	import { VirtualBranch } from '$lib/vbranches/types';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';

	interface Props {
		contextMenuEl?: ReturnType<typeof ContextMenu>;
		target?: HTMLElement;
		headName: string;
		seriesCount: number;
		hasGitHostBranch: boolean;
		hasPr: boolean;
		addDescription: () => void;
		onGenerateBranchName: () => void;
		openPrDetailsModal: () => void;
		reloadPR: () => void;
		onopen?: () => void;
		onclose?: () => void;
	}

	let {
		contextMenuEl = $bindable(),
		target,
		seriesCount,
		hasGitHostBranch,
		headName,
		hasPr,
		addDescription,
		onGenerateBranchName,
		openPrDetailsModal,
		reloadPR,
		onopen,
		onclose
	}: Props = $props();

	const project = getContext(Project);
	const aiService = getContext(AIService);
	const branchStore = getContextStore(VirtualBranch);
	const branchController = getContext(BranchController);
	const aiGenEnabled = projectAiGenEnabled(project.id);

	let deleteSeriesModal: Modal;
	let renameSeriesModal: Modal;
	let newHeadName: string = $state(headName);
	let isDeleting = $state(false);
	let aiConfigurationValid = $state(false);

	$effect(() => {
		setAIConfigurationValid();
	});

	async function setAIConfigurationValid() {
		aiConfigurationValid = await aiService.validateConfiguration();
	}

	const branch = $derived($branchStore);
</script>

<ContextMenu bind:this={contextMenuEl} {target} {onopen} {onclose}>
	<ContextMenuSection>
		<ContextMenuItem
			disabled
			label="Add description"
			onclick={() => {
				addDescription();
				contextMenuEl?.close();
			}}
		/>
		{#if $aiGenEnabled && aiConfigurationValid && !hasGitHostBranch}
			<ContextMenuItem
				label="Generate branch name"
				onclick={() => {
					onGenerateBranchName();
					contextMenuEl?.close();
				}}
			/>
		{/if}
		<ContextMenuItem
			label="Rename"
			onclick={async () => {
				renameSeriesModal.show(branch);
				contextMenuEl?.close();
			}}
		/>
		{#if seriesCount > 1}
			<ContextMenuItem
				label="Delete"
				onclick={() => {
					deleteSeriesModal.show(branch);
					contextMenuEl?.close();
				}}
			/>
		{/if}
	</ContextMenuSection>
	{#if hasPr}
		<ContextMenuSection>
			<ContextMenuItem
				label="PR details"
				onclick={() => {
					openPrDetailsModal();
					contextMenuEl?.close();
				}}
			/>
			<ContextMenuItem
				label="Refetch PR status"
				onclick={() => {
					reloadPR();
					contextMenuEl?.close();
				}}
			/>
			<ContextMenuItem
				label="Copy PR link"
				onclick={() => {
					reloadPR?.();
					contextMenuEl?.close();
				}}
			/>
		</ContextMenuSection>
	{/if}
</ContextMenu>

<Modal
	width="small"
	title={hasGitHostBranch ? 'Branch has already been pushed' : 'Rename branch'}
	type={hasGitHostBranch ? 'warning' : 'info'}
	bind:this={renameSeriesModal}
	onSubmit={(close) => {
		if (newHeadName && newHeadName !== headName) {
			branchController.updateSeriesName(branch.id, headName, newHeadName);
		}
		close();
	}}
>
	<TextBox placeholder="New name" id="newSeriesName" bind:value={newHeadName} focus />

	{#if hasGitHostBranch}
		<div class="text-12 text-light helper-text">
			Renaming a branch that has already been pushed will create a new branch at the remote. The old
			one will remain untouched but will be disassociated from this branch.
		</div>
	{/if}

	{#snippet controls(close)}
		<Button style="ghost" outline type="reset" onclick={close}>Cancel</Button>
		<Button style="pop" kind="solid" type="submit">Rename</Button>
	{/snippet}
</Modal>

<Modal
	width="small"
	title="Delete branch"
	bind:this={deleteSeriesModal}
	onSubmit={async (close) => {
		try {
			isDeleting = true;
			await branchController.removePatchSeries(branch.id, headName);
			close();
		} finally {
			isDeleting = false;
		}
	}}
>
	{#snippet children()}
		Are you sure you want to delete <code class="code-string">{headName}</code>?
	{/snippet}
	{#snippet controls(close)}
		<Button style="ghost" outline onclick={close}>Cancel</Button>
		<Button style="error" kind="solid" type="submit" loading={isDeleting}>Delete</Button>
	{/snippet}
</Modal>

<style>
	.helper-text {
		margin-top: 1rem;
		color: var(--clr-scale-ntrl-50);
		line-height: 1.5;
	}
</style>

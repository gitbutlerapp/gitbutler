<script lang="ts">
	import { AIService } from '$lib/ai/service';
	import { Project } from '$lib/backend/projects';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { copyToClipboard } from '$lib/utils/clipboard';
	import { openExternalUrl } from '$lib/utils/url';
	import { BranchController } from '$lib/vbranches/branchController';
	import { VirtualBranch, type CommitStatus } from '$lib/vbranches/types';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';

	interface Props {
		contextMenuEl?: ReturnType<typeof ContextMenu>;
		leftClickTrigger?: HTMLElement;
		rightClickTrigger?: HTMLElement;
		headName: string;
		seriesCount: number;
		hasForgeBranch: boolean;
		prUrl?: string;
		branchType: CommitStatus;
		description: string;
		toggleDescription: () => Promise<void>;
		onGenerateBranchName: () => void;
		openPrDetailsModal: () => void;
		onAddDependentSeries?: () => void;
		onOpenInBrowser?: () => void;
		reloadPR: () => void;
		onMenuToggle?: (isOpen: boolean, isLeftClick: boolean) => void;
	}

	let {
		contextMenuEl = $bindable(),
		leftClickTrigger,
		rightClickTrigger,
		seriesCount,
		hasForgeBranch,
		headName,
		prUrl,
		branchType,
		description,
		toggleDescription,
		onGenerateBranchName,
		openPrDetailsModal,
		onAddDependentSeries,
		onOpenInBrowser,
		reloadPR,
		onMenuToggle
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

	export function showSeriesRenameModal() {
		renameSeriesModal.show(branch);
	}

	let isOpenedByMouse = $state(false);
</script>

<ContextMenu
	bind:this={contextMenuEl}
	{leftClickTrigger}
	{rightClickTrigger}
	ontoggle={(isOpen, isLeftClick) => {
		if (!isLeftClick) {
			isOpenedByMouse = true;
		} else {
			isOpenedByMouse = false;
		}

		onMenuToggle?.(isOpen, isLeftClick);
	}}
>
	<ContextMenuSection>
		{#if isOpenedByMouse}
			<ContextMenuItem
				label="Add dependent branch"
				onclick={() => {
					onAddDependentSeries?.();
					contextMenuEl?.close();
				}}
			/>
			<ContextMenuItem
				label="Open in browser"
				onclick={() => {
					onOpenInBrowser?.();
					contextMenuEl?.close();
				}}
			/>
		{/if}
		<ContextMenuItem
			label={`${!description ? 'Add' : 'Remove'} description`}
			onclick={async () => {
				await toggleDescription();
				contextMenuEl?.close();
			}}
		/>
		{#if $aiGenEnabled && aiConfigurationValid && !hasForgeBranch}
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
			disabled={branchType === 'integrated'}
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
	{#if prUrl}
		<ContextMenuSection>
			<ContextMenuItem
				label="Open PR in browser"
				onclick={() => {
					openExternalUrl(prUrl);
					contextMenuEl?.close();
				}}
			/>
			<ContextMenuItem
				label="Copy PR link"
				onclick={() => {
					copyToClipboard(prUrl);
					contextMenuEl?.close();
				}}
			/>
			<ContextMenuItem
				label="Show PR details"
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
		</ContextMenuSection>
	{/if}
</ContextMenu>

<Modal
	width="small"
	title={hasForgeBranch ? 'Branch has already been pushed' : 'Rename branch'}
	type={hasForgeBranch ? 'warning' : 'info'}
	bind:this={renameSeriesModal}
	onSubmit={(close) => {
		if (newHeadName && newHeadName !== headName) {
			branchController.updateSeriesName(branch.id, headName, newHeadName);
		}
		close();
	}}
>
	<Textbox placeholder="New name" id="newSeriesName" bind:value={newHeadName} autofocus />

	{#if hasForgeBranch}
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

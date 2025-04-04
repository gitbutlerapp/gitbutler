<script lang="ts">
	import { AIService } from '$lib/ai/service';
	import { writeClipboard } from '$lib/backend/clipboard';
	import { type CommitStatus } from '$lib/commits/commit';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { openExternalUrl } from '$lib/utils/url';
	import { inject } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import type { DetailedPullRequest } from '$lib/forge/interface/types';

	interface Props {
		projectId: string;
		contextMenuEl?: ReturnType<typeof ContextMenu>;
		leftClickTrigger?: HTMLElement;
		rightClickTrigger?: HTMLElement;
		branchName: string;
		seriesCount: number;
		isTopBranch: boolean;
		hasForgeBranch: boolean;
		pr?: DetailedPullRequest;
		branchType: CommitStatus;
		descriptionOption?: boolean;
		descriptionString?: string;
		stackId: string;
		toggleDescription?: () => Promise<void>;
		onGenerateBranchName: () => void;
		onAddDependentSeries?: () => void;
		onOpenInBrowser?: () => void;
		onToggle?: (isOpen: boolean, isLeftClick: boolean) => void;
	}

	let {
		projectId,
		contextMenuEl = $bindable(),
		leftClickTrigger,
		rightClickTrigger,
		isTopBranch,
		seriesCount,
		hasForgeBranch,
		branchName,
		pr,
		branchType,
		descriptionOption = true,
		descriptionString,
		stackId,
		toggleDescription,
		onGenerateBranchName,
		onAddDependentSeries,
		onOpenInBrowser,
		onToggle
	}: Props = $props();

	const [aiService, stackService] = inject(AIService, StackService);
	const aiGenEnabled = $derived(projectAiGenEnabled(projectId));

	const [renameBranch, branchRenameOp] = stackService.updateBranchName;
	const [removeBranch, branchRemovalOp] = stackService.removeBranch;

	let deleteSeriesModal: Modal;
	let renameSeriesModal: Modal;
	let newName: string = $state(branchName);
	let aiConfigurationValid = $state(false);

	$effect(() => {
		setAIConfigurationValid();
	});

	async function setAIConfigurationValid() {
		aiConfigurationValid = await aiService.validateConfiguration();
	}

	export function showSeriesRenameModal() {
		renameSeriesModal.show();
	}
</script>

<ContextMenu
	bind:this={contextMenuEl}
	{leftClickTrigger}
	{rightClickTrigger}
	ontoggle={(isOpen, isLeftClick) => {
		onToggle?.(isOpen, isLeftClick);
	}}
>
	{#if isTopBranch}
		<ContextMenuSection>
			<ContextMenuItem
				label="Add dependent branch"
				onclick={() => {
					onAddDependentSeries?.();
					contextMenuEl?.close();
				}}
			/>
		</ContextMenuSection>
	{/if}
	<ContextMenuSection>
		{#if hasForgeBranch}
			<ContextMenuItem
				label="Open in browser"
				onclick={() => {
					onOpenInBrowser?.();
					contextMenuEl?.close();
				}}
			/>
		{/if}
		<ContextMenuItem
			label="Copy branch name"
			onclick={() => {
				writeClipboard(branchName);
				contextMenuEl?.close();
			}}
		/>
	</ContextMenuSection>
	<ContextMenuSection>
		{#if descriptionOption}
			<ContextMenuItem
				label={`${!descriptionString ? 'Add' : 'Remove'} description`}
				onclick={async () => {
					await toggleDescription?.();
					contextMenuEl?.close();
				}}
			/>
		{/if}
		{#if $aiGenEnabled && aiConfigurationValid && !hasForgeBranch}
			<ContextMenuItem
				label="Generate branch name"
				onclick={() => {
					onGenerateBranchName();
					contextMenuEl?.close();
				}}
			/>
		{/if}
		{#if branchType !== 'Integrated'}
			<ContextMenuItem
				label="Rename"
				onclick={async () => {
					renameSeriesModal.show(stackId);
					contextMenuEl?.close();
				}}
			/>
		{/if}
		{#if seriesCount > 1}
			<ContextMenuItem
				label="Delete"
				onclick={() => {
					deleteSeriesModal.show(stackId);
					contextMenuEl?.close();
				}}
			/>
		{/if}
	</ContextMenuSection>
	{#if pr?.htmlUrl}
		<ContextMenuSection>
			<ContextMenuItem
				label="Open PR in browser"
				onclick={() => {
					openExternalUrl(pr.htmlUrl);
					contextMenuEl?.close();
				}}
			/>
			<ContextMenuItem
				label="Copy PR link"
				onclick={() => {
					writeClipboard(pr.htmlUrl);
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
	onSubmit={async (close) => {
		if (newName && newName !== branchName) {
			await renameBranch({
				projectId,
				stackId,
				branchName,
				newName
			});
		}
		close();
	}}
>
	<Textbox placeholder="New name" id="newSeriesName" bind:value={newName} autofocus />

	{#if hasForgeBranch}
		<div class="text-12 helper-text">
			Renaming a branch that has already been pushed will create a new branch at the remote. The old
			one will remain untouched but will be disassociated from this branch.
		</div>
	{/if}

	{#snippet controls(close)}
		<Button kind="outline" type="reset" onclick={close}>Cancel</Button>
		<Button style="pop" type="submit" loading={branchRenameOp.current.isLoading}>Rename</Button>
	{/snippet}
</Modal>

<Modal
	width="small"
	title="Delete branch"
	bind:this={deleteSeriesModal}
	onSubmit={async (close) => {
		await removeBranch({
			projectId,
			stackId,
			branchName
		});
		close();
	}}
>
	{#snippet children()}
		Are you sure you want to delete <code class="code-string">{branchName}</code>?
	{/snippet}
	{#snippet controls(close)}
		<Button kind="outline" onclick={close}>Cancel</Button>
		<Button style="error" type="submit" loading={branchRemovalOp.current.isLoading}>Delete</Button>
	{/snippet}
</Modal>

<style>
	.helper-text {
		margin-top: 1rem;
		color: var(--clr-scale-ntrl-50);
		line-height: 1.5;
	}
</style>

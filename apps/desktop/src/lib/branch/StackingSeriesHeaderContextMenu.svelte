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
		disableTitleEdit: boolean;
		hasPr: boolean;
		addDescription: () => void;
		onGenerateBranchName: () => void;
		openPrDetailsModal: () => void;
		reloadPR: () => void;
	}

	let {
		contextMenuEl = $bindable(),
		target,
		seriesCount,
		disableTitleEdit,
		headName,
		hasPr,
		addDescription,
		onGenerateBranchName,
		openPrDetailsModal,
		reloadPR
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

<ContextMenu bind:this={contextMenuEl} {target}>
	<ContextMenuSection>
		<ContextMenuItem
			disabled
			label="Add description"
			on:click={() => {
				addDescription();
				contextMenuEl?.close();
			}}
		/>
		<ContextMenuItem
			label="Generate series name"
			on:click={() => {
				onGenerateBranchName();
				contextMenuEl?.close();
			}}
			disabled={!($aiGenEnabled && aiConfigurationValid) || disableTitleEdit}
		/>
		<ContextMenuItem
			label="Rename"
			on:click={async () => {
				renameSeriesModal.show(branch);
				contextMenuEl?.close();
			}}
		/>
		<ContextMenuItem
			label="Delete"
			disabled={seriesCount <= 1}
			on:click={() => {
				deleteSeriesModal.show(branch);
				contextMenuEl?.close();
			}}
		/>
	</ContextMenuSection>
	{#if hasPr}
		<ContextMenuSection>
			<ContextMenuItem
				label="PR details"
				on:click={() => {
					openPrDetailsModal();
					contextMenuEl?.close();
				}}
			/>
			<ContextMenuItem
				label="Refetch PR status"
				on:click={() => {
					reloadPR();
					contextMenuEl?.close();
				}}
			/>
		</ContextMenuSection>
	{/if}
</ContextMenu>

<Modal
	width="small"
	title="Rename series"
	bind:this={renameSeriesModal}
	onSubmit={(close) => {
		if (newHeadName && newHeadName !== headName) {
			branchController.updateSeriesName(branch.id, headName, newHeadName);
		}
		close();
	}}
>
	<TextBox placeholder="New name" id="newSeriesName" bind:value={newHeadName} focus />

	{#snippet controls(close)}
		<Button style="ghost" outline type="reset" onclick={close}>Cancel</Button>
		<Button style="pop" kind="solid" type="submit">Rename</Button>
	{/snippet}
</Modal>

<Modal
	width="small"
	title="Delete series"
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
	{#snippet children(branch)}
		Are you sure you want to delete <code class="code-string">{branch.name}</code>?
	{/snippet}
	{#snippet controls(close)}
		<Button style="ghost" outline onclick={close}>Cancel</Button>
		<Button style="error" kind="solid" type="submit" loading={isDeleting}>Delete</Button>
	{/snippet}
</Modal>

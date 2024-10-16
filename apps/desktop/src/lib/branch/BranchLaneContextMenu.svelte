<script lang="ts">
	import { AIService } from '$lib/ai/service';
	import { Project } from '$lib/backend/projects';
	import { getNameNormalizationServiceContext } from '$lib/branches/nameNormalizationService';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { projectAiGenEnabled } from '$lib/config/config';
	import { stackingFeature } from '$lib/config/uiFeatureFlags';
	import TextBox from '$lib/shared/TextBox.svelte';
	import Toggle from '$lib/shared/Toggle.svelte';
	import { BranchController } from '$lib/vbranches/branchController';
	import { VirtualBranch } from '$lib/vbranches/types';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';

	interface Props {
		hasPr: boolean;
		contextMenuEl?: ReturnType<typeof ContextMenu>;
		target?: HTMLElement;
		onCollapse: () => void;
		onGenerateBranchName?: () => void;
		openPrDetailsModal?: () => void;
		reloadPR?: () => void;
	}

	let {
		contextMenuEl = $bindable(),
		target,
		onCollapse,
		onGenerateBranchName,
		hasPr,
		openPrDetailsModal,
		reloadPR
	}: Props = $props();

	const project = getContext(Project);
	const aiService = getContext(AIService);
	const branchStore = getContextStore(VirtualBranch);
	const aiGenEnabled = projectAiGenEnabled(project.id);
	const branchController = getContext(BranchController);

	const nameNormalizationService = getNameNormalizationServiceContext();

	let deleteBranchModal: Modal;
	let renameRemoteModal: Modal;
	let aiConfigurationValid = $state(false);
	let newRemoteName = $state('');
	let allowRebasing = $state<boolean>();
	let isDeleting = $state(false);

	const branch = $derived($branchStore);
	const commits = $derived(branch.commits);
	$effect(() => {
		allowRebasing = branch.allowRebasing;
	});

	$effect(() => {
		setAIConfigurationValid();
	});

	async function toggleAllowRebasing() {
		branchController.updateBranchAllowRebasing(branch.id, !allowRebasing);
	}

	async function setAIConfigurationValid() {
		aiConfigurationValid = await aiService.validateConfiguration();
	}

	function saveAndUnapply() {
		branchController.saveAndUnapply(branch.id);
	}

	let normalizedBranchName: string;

	$effect(() => {
		if (branch.name) {
			nameNormalizationService
				.normalize(branch.name)
				.then((name) => {
					normalizedBranchName = name;
				})
				.catch((e) => {
					console.error('Failed to normalize branch name', e);
				});
		}
	});
</script>

<ContextMenu bind:this={contextMenuEl} {target}>
	<ContextMenuSection>
		<ContextMenuItem
			label="Collapse lane"
			on:click={() => {
				onCollapse();
				contextMenuEl?.close();
			}}
		/>
	</ContextMenuSection>
	<ContextMenuSection>
		<ContextMenuItem
			label="Unapply"
			on:click={async () => {
				if (commits.length === 0 && branch.files?.length === 0) {
					await branchController.unapplyWithoutSaving(branch.id);
				} else {
					saveAndUnapply();
				}
				contextMenuEl?.close();
			}}
		/>

		<ContextMenuItem
			label="Unapply and drop changes"
			on:click={async () => {
				if (
					branch.name.toLowerCase().includes('virtual branch') &&
					commits.length === 0 &&
					branch.files?.length === 0
				) {
					await branchController.unapplyWithoutSaving(branch.id);
				} else {
					deleteBranchModal.show(branch);
				}
				contextMenuEl?.close();
			}}
		/>

		{#if !$stackingFeature}
			<ContextMenuItem
				label="Generate branch name"
				on:click={() => {
					onGenerateBranchName?.();
					contextMenuEl?.close();
				}}
				disabled={!($aiGenEnabled && aiConfigurationValid) || branch.files?.length === 0}
			/>
		{/if}
	</ContextMenuSection>

	{#if !$stackingFeature}
		<ContextMenuSection>
			<ContextMenuItem
				label="Set remote branch name"
				on:click={() => {
					newRemoteName = branch.upstreamName || normalizedBranchName || '';
					renameRemoteModal.show(branch);
					contextMenuEl?.close();
				}}
			/>
		</ContextMenuSection>
	{/if}

	<ContextMenuSection>
		<ContextMenuItem label="Allow rebasing" on:click={toggleAllowRebasing}>
			<Tooltip slot="control" text={'Allows changing commits after push\n(force push needed)'}>
				<Toggle small bind:checked={allowRebasing} on:click={toggleAllowRebasing} />
			</Tooltip>
		</ContextMenuItem>
	</ContextMenuSection>

	{#if !$stackingFeature && hasPr}
		<ContextMenuSection>
			<ContextMenuItem
				label="PR details"
				on:click={() => {
					openPrDetailsModal?.();
					contextMenuEl?.close();
				}}
			/>
			<ContextMenuItem
				label="Refetch PR status"
				on:click={() => {
					reloadPR?.();
					contextMenuEl?.close();
				}}
			/>
		</ContextMenuSection>
	{/if}

	<ContextMenuSection>
		<ContextMenuItem
			label={`Create ${$stackingFeature ? 'stack' : 'branch'} to the left`}
			on:click={() => {
				branchController.createBranch({ order: branch.order });
				contextMenuEl?.close();
			}}
		/>

		<ContextMenuItem
			label={`Create ${$stackingFeature ? 'stack' : 'branch'} to the right`}
			on:click={() => {
				branchController.createBranch({ order: branch.order + 1 });
				contextMenuEl?.close();
			}}
		/>
	</ContextMenuSection>
</ContextMenu>

<Modal
	width="small"
	bind:this={renameRemoteModal}
	onSubmit={(close) => {
		branchController.updateBranchRemoteName(branch.id, newRemoteName);
		close();
	}}
>
	<TextBox label="Remote branch name" id="newRemoteName" bind:value={newRemoteName} focus />

	{#snippet controls(close)}
		<Button style="ghost" outline type="reset" onclick={close}>Cancel</Button>
		<Button style="pop" kind="solid" type="submit">Rename</Button>
	{/snippet}
</Modal>

<Modal
	width="small"
	bind:this={deleteBranchModal}
	onSubmit={async (close) => {
		try {
			isDeleting = true;
			await branchController.unapplyWithoutSaving(branch.id);
			close();
		} finally {
			isDeleting = false;
		}
	}}
>
	{#snippet children(branch)}
		All changes will be lost for <strong>{branch.name}</strong>. Are you sure you want to continue?
	{/snippet}
	{#snippet controls(close)}
		<Button style="ghost" outline onclick={close}>Cancel</Button>
		<Button style="error" kind="solid" type="submit" loading={isDeleting}
			>Unapply and drop changes</Button
		>
	{/snippet}
</Modal>

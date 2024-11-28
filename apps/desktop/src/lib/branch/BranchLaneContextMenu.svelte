<script lang="ts">
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { getForgePrService } from '$lib/forge/interface/forgePrService';
	import { updatePrDescriptionTables } from '$lib/forge/shared/prFooter';
	import { User } from '$lib/stores/user';
	import { BranchController } from '$lib/vbranches/branchController';
	import { VirtualBranch } from '$lib/vbranches/types';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Toggle from '@gitbutler/ui/Toggle.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';

	interface Props {
		prUrl?: string;
		contextMenuEl?: ReturnType<typeof ContextMenu>;
		trigger?: HTMLElement;
		onCollapse: () => void;
		onGenerateBranchName?: () => void;
		openPrDetailsModal?: () => void;
		reloadPR?: () => void;
		ontoggle?: (isOpen: boolean) => void;
	}

	let { contextMenuEl = $bindable(), trigger, onCollapse, ontoggle }: Props = $props();

	const branchStore = getContextStore(VirtualBranch);
	const branchController = getContext(BranchController);
	const prService = getForgePrService();
	const user = getContextStore(User);

	let deleteBranchModal: Modal;
	let allowRebasing = $state<boolean>();
	let isDeleting = $state(false);

	const branch = $derived($branchStore);
	const commits = $derived(branch.validSeries.flatMap((s) => s.patches));

	$effect(() => {
		allowRebasing = branch.allowRebasing;
	});

	const allPrIds = $derived(branch.validSeries.map((series) => series.prNumber).filter(isDefined));

	async function toggleAllowRebasing() {
		branchController.updateBranchAllowRebasing(branch.id, !allowRebasing);
	}

	function saveAndUnapply() {
		branchController.saveAndUnapply(branch.id);
	}
</script>

<ContextMenu bind:this={contextMenuEl} leftClickTrigger={trigger} {ontoggle}>
	<ContextMenuSection>
		<ContextMenuItem
			label="Collapse lane"
			onclick={() => {
				onCollapse();
				contextMenuEl?.close();
			}}
		/>
	</ContextMenuSection>
	<ContextMenuSection>
		<ContextMenuItem
			label="Unapply"
			onclick={async () => {
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
			onclick={async () => {
				if (
					branch.name.toLowerCase().includes('lane') &&
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
	</ContextMenuSection>

	<ContextMenuSection>
		<ContextMenuItem label="Allow rebasing" onclick={toggleAllowRebasing}>
			{#snippet control()}
				<Tooltip text={'Allows changing commits after push\n(force push needed)'}>
					<Toggle small bind:checked={allowRebasing} onclick={toggleAllowRebasing} />
				</Tooltip>
			{/snippet}
		</ContextMenuItem>
	</ContextMenuSection>

	<ContextMenuSection>
		<ContextMenuItem
			label={`Create stack to the left`}
			onclick={() => {
				branchController.createBranch({ order: branch.order });
				contextMenuEl?.close();
			}}
		/>

		<ContextMenuItem
			label={`Create stack to the right`}
			onclick={() => {
				branchController.createBranch({ order: branch.order + 1 });
				contextMenuEl?.close();
			}}
		/>
	</ContextMenuSection>
	{#if $user && $user.role?.includes('admin')}
		<!-- TODO: Remove after iterating on the pull request footer. -->
		<ContextMenuSection title="Admin only">
			<ContextMenuItem
				label="Update PR footers"
				disabled={allPrIds.length === 0}
				onclick={() => {
					if ($prService && branch) {
						const allPrIds = branch.validSeries.map((series) => series.prNumber).filter(isDefined);
						updatePrDescriptionTables($prService, allPrIds);
					}
					contextMenuEl?.close();
				}}
			/>
		</ContextMenuSection>
	{/if}
</ContextMenu>

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

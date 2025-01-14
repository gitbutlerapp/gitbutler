<script lang="ts">
	import ContextMenu from '$components/ContextMenu.svelte';
	import ContextMenuItem from '$components/ContextMenuItem.svelte';
	import ContextMenuSection from '$components/ContextMenuSection.svelte';
	import { getForgePrService } from '$lib/forge/interface/forgePrService';
	import { updatePrDescriptionTables } from '$lib/forge/shared/prFooter';
	import { User } from '$lib/stores/user';
	import { BranchController } from '$lib/vbranches/branchController';
	import { BranchStack } from '$lib/vbranches/types';
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

	const branchStore = getContextStore(BranchStack);
	const branchController = getContext(BranchController);
	const prService = getForgePrService();
	const user = getContextStore(User);

	let deleteBranchModal: Modal;
	let allowRebasing = $state<boolean>();
	let isDeleting = $state(false);

	const stack = $derived($branchStore);
	const commits = $derived(stack.validSeries.flatMap((s) => s.patches));

	$effect(() => {
		allowRebasing = stack.allowRebasing;
	});

	const allPrIds = $derived(stack.validSeries.map((series) => series.prNumber).filter(isDefined));

	async function toggleAllowRebasing() {
		branchController.updateBranchAllowRebasing(stack.id, !allowRebasing);
	}

	function saveAndUnapply() {
		branchController.saveAndUnapply(stack.id);
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
				if (commits.length === 0 && stack.files?.length === 0) {
					await branchController.unapplyWithoutSaving(stack.id);
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
					stack.name.toLowerCase().includes('lane') &&
					commits.length === 0 &&
					stack.files?.length === 0
				) {
					await branchController.unapplyWithoutSaving(stack.id);
				} else {
					deleteBranchModal.show(stack);
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
				branchController.createBranch({ order: stack.order });
				contextMenuEl?.close();
			}}
		/>

		<ContextMenuItem
			label={`Create stack to the right`}
			onclick={() => {
				branchController.createBranch({ order: stack.order + 1 });
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
					if ($prService && stack) {
						const allPrIds = stack.validSeries.map((series) => series.prNumber).filter(isDefined);
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
			await branchController.unapplyWithoutSaving(stack.id);
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
		<Button kind="outline" onclick={close}>Cancel</Button>
		<Button style="error" type="submit" loading={isDeleting}>Unapply and drop changes</Button>
	{/snippet}
</Modal>

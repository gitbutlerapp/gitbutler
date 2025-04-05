<script lang="ts">
	import { BranchStack } from '$lib/branches/branch';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { updatePrDescriptionTables } from '$lib/forge/shared/prFooter';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { User } from '$lib/user/user';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Toggle from '@gitbutler/ui/Toggle.svelte';
	import Tooltip from '@gitbutler/ui/Tooltip.svelte';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';

	interface Props {
		projectId: string;
		contextMenuEl?: ReturnType<typeof ContextMenu>;
		trigger?: HTMLElement;
		onCollapse: () => void;
		ontoggle?: (isOpen: boolean) => void;
	}

	let { projectId, contextMenuEl = $bindable(), trigger, onCollapse, ontoggle }: Props = $props();

	const branchStore = getContextStore(BranchStack);
	const stackService = getContext(StackService);
	const forge = getContext(DefaultForgeFactory);
	const prService = $derived(forge.current.prService);
	const user = getContextStore(User);

	const [unapply] = stackService.unapply;
	const [updateStack] = stackService.updateStack;
	const [newStack] = stackService.newStack;

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
		updateStack({
			projectId,
			branch: {
				id: stack.id,
				allow_rebasing: !allowRebasing
			}
		});
	}

	function saveAndUnapply() {
		unapply({
			projectId: projectId,
			stackId: stack.id
		});
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
					await unapply({
						projectId: projectId,
						stackId: stack.id
					});
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
					await unapply({
						projectId: projectId,
						stackId: stack.id
					});
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
				newStack({
					projectId,
					branch: { order: stack.order }
				});
				contextMenuEl?.close();
			}}
		/>

		<ContextMenuItem
			label={`Create stack to the right`}
			onclick={() => {
				newStack({
					projectId,
					branch: { order: stack.order + 1 }
				});
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
					if (prService && stack) {
						const allPrIds = stack.validSeries.map((series) => series.prNumber).filter(isDefined);
						updatePrDescriptionTables(prService, allPrIds);
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
			const [unapply] = stackService.unapply;
			await unapply({
				projectId,
				stackId: stack.id
			});
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

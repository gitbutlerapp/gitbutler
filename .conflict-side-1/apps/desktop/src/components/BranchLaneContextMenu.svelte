<script lang="ts">
	import { BranchStack } from '$lib/branches/branch';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { updatePrDescriptionTables } from '$lib/forge/shared/prFooter';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { User } from '$lib/user/user';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import ContextMenu from '@gitbutler/ui/ContextMenu.svelte';
	import ContextMenuItem from '@gitbutler/ui/ContextMenuItem.svelte';
	import ContextMenuSection from '@gitbutler/ui/ContextMenuSection.svelte';
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

	let allowRebasing = $state<boolean>();

	const stack = $derived($branchStore);

	$effect(() => {
		allowRebasing = stack.allowRebasing;
	});

	const allPrIds = $derived(stack.validSeries.map((series) => series.prNumber).filter(isDefined));

	async function toggleAllowRebasing() {
		await stackService.updateStack({
			projectId,
			branch: {
				id: stack.id,
				allow_rebasing: !allowRebasing
			}
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
				await stackService.unapply({
					projectId: projectId,
					stackId: stack.id
				});
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
				stackService.newStackMutation({
					projectId,
					branch: { order: stack.order }
				});
				contextMenuEl?.close();
			}}
		/>

		<ContextMenuItem
			label={`Create stack to the right`}
			onclick={() => {
				stackService.newStackMutation({
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

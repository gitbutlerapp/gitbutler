<script lang="ts" context="module">
	export enum BranchAction {
		Push = 'push',
		Rebase = 'rebase'
	}
</script>

<script lang="ts">
	import { Project } from '$lib/backend/projects';
	import DropDownButton from '$lib/components/DropDownButton.svelte';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { persisted, type Persisted } from '$lib/persisted/persisted';
	import { getContext } from '$lib/utils/context';
	import { getLocalCommits, getUnknownCommits } from '$lib/vbranches/contexts';
	import { createEventDispatcher } from 'svelte';
	import type { Branch } from '$lib/vbranches/types';

	export let isLoading = false;
	export let wide = false;
	export let branch: Branch;

	const project = getContext(Project);
	const localCommits = getLocalCommits();
	const unknownCommits = getUnknownCommits();

	function defaultAction(): Persisted<BranchAction> {
		const key = 'projectDefaultAction_';
		return persisted<BranchAction>(BranchAction.Push, key + project.id);
	}

	const dispatch = createEventDispatcher<{ trigger: { action: BranchAction } }>();
	const preferredAction = defaultAction();

	let contextMenu: ContextMenu;
	let dropDown: DropDownButton;
	let disabled = false;
	let isPushed = $localCommits.length === 0 && !branch.requiresForce;
	$: canBeRebased = $unknownCommits.length > 0;
	$: action = selectAction(isPushed, $preferredAction);

	function selectAction(isPushed: boolean, preferredAction: BranchAction) {
		// TODO: Refactor such that this is not necessary
		if (isPushed) {
			return BranchAction.Rebase;
		} else if (!branch.requiresForce) {
			return BranchAction.Push;
		}
		return preferredAction;
	}

	$: pushLabel = branch.requiresForce ? 'Force push' : 'Push';

	$: labels = {
		[BranchAction.Push]: pushLabel,
		[BranchAction.Rebase]: 'Rebase branch'
	};
</script>

<DropDownButton
	style="pop"
	kind="soft"
	loading={isLoading}
	bind:this={dropDown}
	{wide}
	{disabled}
	menuPosition="top"
	on:click={() => {
		dispatch('trigger', { action });
	}}
>
	{labels[$preferredAction]}
	<ContextMenu slot="context-menu" bind:this={contextMenu}>
		<ContextMenuSection>
			{#if !isPushed}
				<ContextMenuItem
					label={labels[BranchAction.Push]}
					disabled={isPushed}
					on:click={() => {
						$preferredAction = BranchAction.Push;
						dropDown.close();
					}}
				/>
			{/if}
			{#if !branch.requiresForce || canBeRebased}
				<ContextMenuItem
					label={labels[BranchAction.Rebase]}
					disabled={isPushed || $unknownCommits.length === 0}
					on:click={() => {
						$preferredAction = BranchAction.Rebase;
						dropDown.close();
					}}
				/>
			{/if}
		</ContextMenuSection>
	</ContextMenu>
</DropDownButton>

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
	import * as toasts from '$lib/utils/toasts';
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
	$: selection$ = contextMenu?.selection$;
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
</script>

<DropDownButton
	style="pop"
	kind="soft"
	loading={isLoading}
	bind:this={dropDown}
	{wide}
	{disabled}
	dropdownPosition="top"
	on:click={() => {
		dispatch('trigger', { action });
	}}
>
	{$selection$?.label}
	<ContextMenu
		type="select"
		slot="context-menu"
		bind:this={contextMenu}
		on:select={(e) => {
			// TODO: Refactor to use generics if/when that works with Svelte
			switch (e.detail?.id) {
				case BranchAction.Push:
					$preferredAction = BranchAction.Push;
					break;
				case BranchAction.Rebase:
					$preferredAction = BranchAction.Rebase;
					break;
				default:
					toasts.error('Uknown branch action');
			}
			dropDown.close();
		}}
	>
		<ContextMenuSection>
			{#if !isPushed}
				<ContextMenuItem
					id="push"
					label={pushLabel}
					selected={action === BranchAction.Push}
					disabled={isPushed}
				/>
			{/if}
			{#if !branch.requiresForce || canBeRebased}
				<ContextMenuItem
					id="rebase"
					label="Rebase upstream"
					selected={action === BranchAction.Rebase}
					disabled={isPushed || $unknownCommits.length === 0}
				/>
			{/if}
		</ContextMenuSection>
	</ContextMenu>
</DropDownButton>

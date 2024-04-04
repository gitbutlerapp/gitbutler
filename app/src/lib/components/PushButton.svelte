<script lang="ts" context="module">
	export enum BranchAction {
		Push = 'push',
		Pr = 'pr',
		DraftPr = 'draftPr'
	}
</script>

<script lang="ts">
	import { Project } from '$lib/backend/projects';
	import Button from '$lib/components/Button.svelte';
	import DropDownButton from '$lib/components/DropDownButton.svelte';
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { persisted, type Persisted } from '$lib/persisted/persisted';
	import { getContext } from '$lib/utils/context';
	import * as toasts from '$lib/utils/toasts';
	import { createEventDispatcher } from 'svelte';
	import type { Branch } from '$lib/vbranches/types';

	export let type: string;
	export let isLoading = false;
	export let githubEnabled: boolean;
	export let wide = false;
	export let branch: Branch;
	export let isPr = false;

	const project = getContext(Project);

	function defaultAction(): Persisted<BranchAction> {
		const key = 'projectDefaultAction_';
		return persisted<BranchAction>(BranchAction.Push, key + project.id);
	}

	const dispatch = createEventDispatcher<{ trigger: { action: BranchAction } }>();
	const preferredAction = defaultAction();

	let contextMenu: ContextMenu;
	let dropDown: DropDownButton;
	let disabled = false;

	$: selection$ = contextMenu?.selection$;

	let action!: BranchAction;
	$: {
		isPushed; // selectAction is dependant on isPushed
		action = selectAction($preferredAction);
	}

	function selectAction(preferredAction: BranchAction) {
		if (isPushed && !githubEnabled) {
			// TODO: Refactor such that this is not necessary
			console.log('No push actions possible');
			return BranchAction.Push;
		} else if (isPushed) {
			if (preferredAction == BranchAction.Push) return BranchAction.Pr;
			return preferredAction;
		} else if (!githubEnabled) {
			return BranchAction.Push;
		}
		return preferredAction;
	}

	$: pushLabel = branch.requiresForce ? 'Force push to remote' : 'Push to remote';
	$: commits = branch.commits.filter((c) => c.status == type);
	$: isPushed = type === 'remote' && !branch.requiresForce;
</script>

{#if (isPr || commits.length === 0) && !isPushed}
	<Button
		style="ghost"
		kind="solid"
		{wide}
		disabled={isPushed}
		loading={isLoading}
		on:click={() => {
			dispatch('trigger', { action: BranchAction.Push });
		}}>{pushLabel}</Button
	>
{:else if !isPr}
	<DropDownButton
		style="ghost"
		kind="solid"
		loading={isLoading}
		bind:this={dropDown}
		{wide}
		{disabled}
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
					case BranchAction.Pr:
						$preferredAction = BranchAction.Pr;
						break;
					case BranchAction.DraftPr:
						$preferredAction = BranchAction.DraftPr;
						break;
					default:
						toasts.error('Uknown branch action');
				}
				dropDown.close();
			}}
		>
			<ContextMenuSection>
				<ContextMenuItem
					id="push"
					label={pushLabel}
					selected={action == BranchAction.Push}
					disabled={isPushed}
				/>
				<ContextMenuItem
					id="pr"
					label="Create pull request"
					disabled={!githubEnabled}
					selected={action == BranchAction.Pr}
				/>
				<ContextMenuItem
					id="draftPr"
					label="Create draft pull request"
					disabled={!githubEnabled}
					selected={action == BranchAction.DraftPr}
				/>
			</ContextMenuSection>
		</ContextMenu>
	</DropDownButton>
{/if}

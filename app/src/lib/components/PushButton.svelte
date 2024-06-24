<script lang="ts" context="module">
	export enum BranchAction {
		Push = 'push',
		Integrate = 'integrate'
	}
</script>

<script lang="ts">
	import ContextMenu from '$lib/components/contextmenu/ContextMenu.svelte';
	import ContextMenuItem from '$lib/components/contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from '$lib/components/contextmenu/ContextMenuSection.svelte';
	import { persisted } from '$lib/persisted/persisted';
	import DropDownButton from '$lib/shared/DropDownButton.svelte';
	import { createEventDispatcher } from 'svelte';

	export let integrate: boolean; // Integrate upstream option enabled
	export let projectId: string;

	export let isLoading = false;
	export let wide = false;
	export let requiresForce: boolean;

	const dispatch = createEventDispatcher<{ trigger: { action: BranchAction } }>();
	const preferredAction = persisted<BranchAction>(
		BranchAction.Push,
		'projectDefaultAction_' + projectId
	);

	let contextMenu: ContextMenu;
	let dropDown: DropDownButton;
	let disabled = false;

	$: action = selectAction($preferredAction);
	$: pushLabel = requiresForce ? 'Force push' : 'Push';
	$: labels = {
		[BranchAction.Push]: pushLabel,
		[BranchAction.Integrate]: 'Integrate upstream'
	};

	function selectAction(preferredAction: BranchAction) {
		if (preferredAction === BranchAction.Integrate && integrate) return BranchAction.Integrate;
		return BranchAction.Push;
	}
</script>

<DropDownButton
	style="pop"
	kind="solid"
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
			<ContextMenuItem
				label={labels[BranchAction.Push]}
				on:click={() => {
					$preferredAction = BranchAction.Push;
					dropDown.close();
				}}
			/>
			<ContextMenuItem
				label={labels[BranchAction.Integrate]}
				disabled={!integrate}
				on:click={() => {
					$preferredAction = BranchAction.Integrate;
					dropDown.close();
				}}
			/>
		</ContextMenuSection>
	</ContextMenu>
</DropDownButton>

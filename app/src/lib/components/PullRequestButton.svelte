<script lang="ts">
	import DropDownButton from './DropDownButton.svelte';
	import ContextMenu from './contextmenu/ContextMenu.svelte';
	import ContextMenuItem from './contextmenu/ContextMenuItem.svelte';
	import ContextMenuSection from './contextmenu/ContextMenuSection.svelte';
	import { persisted, type Persisted } from '$lib/persisted/persisted';
	import { createEventDispatcher } from 'svelte';

	enum Action {
		Create = 'create',
		Draft = 'draft'
	}

	const dispatch = createEventDispatcher<{ click: { action: Action } }>();
	const action = defaultAction();

	export let loading = false;
	let dropDown: DropDownButton;

	const labels = {
		[Action.Create]: 'Create PR',
		[Action.Draft]: 'Create Draft PR'
	};

	function defaultAction(): Persisted<Action> {
		const key = 'projectDefaultPrAction';
		return persisted<Action>(Action.Create, key);
	}
</script>

<DropDownButton
	style="ghost"
	outline
	{loading}
	bind:this={dropDown}
	on:click={() => {
		dispatch('click', { action: $action });
	}}
>
	{labels[$action]}
	<ContextMenu slot="context-menu">
		<ContextMenuSection>
			<ContextMenuItem
				label={labels[Action.Create]}
				on:click={() => {
					$action = Action.Create;
					dropDown.close();
				}}
			/>
			<ContextMenuItem
				label={labels[Action.Draft]}
				on:click={() => {
					$action = Action.Draft;
					dropDown.close();
				}}
			/>
		</ContextMenuSection>
	</ContextMenu>
</DropDownButton>
